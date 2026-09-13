use anyhow::{Context, Result, ensure};
use radio_core::import_types::BeatmapMetadata as ImportedMetadata;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect,
    QueryTrait, sea_query::OnConflict,
};
use sha2::{Digest, Sha256};

use crate::{
    entities::{beatmap, beatmap_metadata},
    model::BeatmapMetadata,
};

pub struct BeatmapMetadataRepository<'a> {
    pub(crate) connection: &'a sea_orm::DatabaseConnection,
}

impl BeatmapMetadataRepository<'_> {
    pub async fn get(&self, hash: &str) -> Result<Option<BeatmapMetadata>> {
        Ok(beatmap_metadata::Entity::find_by_id(hash)
            .one(self.connection)
            .await?
            .map(into_model))
    }

    pub async fn get_or_insert(&self, imported: &ImportedMetadata) -> Result<BeatmapMetadata> {
        get_or_insert(self.connection, imported).await
    }
}

/// Hashes every imported metadata field without normalization or resolved audio paths.
pub fn metadata_hash(imported: &ImportedMetadata) -> Result<String> {
    let author = imported
        .author
        .as_ref()
        .map(|author| (author.online_id, &author.username, &author.country_code));
    let bytes = serde_json::to_vec(&(
        "radio-db:metadata:v1",
        &imported.title,
        &imported.title_unicode,
        &imported.artist,
        &imported.artist_unicode,
        author,
        &imported.source,
        &imported.tags,
        &imported.user_tags,
        imported.preview_time,
        &imported.audio_file,
        &imported.background_file,
    ))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub(crate) async fn get_or_insert(
    connection: &impl ConnectionTrait,
    imported: &ImportedMetadata,
) -> Result<BeatmapMetadata> {
    let expected = beatmap_metadata::Model {
        hash: metadata_hash(imported)?,
        title: imported.title.clone(),
        title_unicode: imported.title_unicode.clone(),
        artist: imported.artist.clone(),
        artist_unicode: imported.artist_unicode.clone(),
        author: imported.author.as_ref().map(|author| {
            serde_json::json!({
                "online_id": author.online_id,
                "username": author.username,
                "country_code": author.country_code,
            })
        }),
        source: imported.source.clone(),
        tags: imported.tags.clone(),
        user_tags: serde_json::to_value(&imported.user_tags)?,
        preview_time: imported.preview_time,
        audio_file: imported.audio_file.clone(),
        background_file: imported.background_file.clone(),
    };
    beatmap_metadata::Entity::insert(expected.clone().into_active_model())
        .on_conflict(
            OnConflict::column(beatmap_metadata::Column::Hash)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(connection)
        .await?;
    let actual = beatmap_metadata::Entity::find_by_id(&expected.hash)
        .one(connection)
        .await?
        .context("metadata missing after insertion")?;
    ensure!(
        actual == expected,
        "metadata hash collision or corrupted content for {}",
        expected.hash
    );
    Ok(into_model(actual))
}

pub(crate) async fn cleanup(connection: &impl ConnectionTrait) -> Result<()> {
    beatmap_metadata::Entity::delete_many()
        .filter(
            beatmap_metadata::Column::Hash.not_in_subquery(
                beatmap::Entity::find()
                    .select_only()
                    .column(beatmap::Column::MetadataHash)
                    .filter(beatmap::Column::MetadataHash.is_not_null())
                    .into_query(),
            ),
        )
        .exec(connection)
        .await?;
    Ok(())
}

fn into_model(value: beatmap_metadata::Model) -> BeatmapMetadata {
    BeatmapMetadata {
        hash: value.hash,
        title: value.title,
        title_unicode: value.title_unicode,
        artist: value.artist,
        artist_unicode: value.artist_unicode,
        author: value.author,
        source: value.source,
        tags: value.tags,
        user_tags: value.user_tags,
        preview_time: value.preview_time,
        audio_file: value.audio_file,
        background_file: value.background_file,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::import_types::RealmUser;

    fn fixture() -> ImportedMetadata {
        ImportedMetadata {
            title: Some("Title".into()),
            title_unicode: Some("タイトル".into()),
            artist: Some("Artist".into()),
            artist_unicode: Some("アーティスト".into()),
            author: Some(RealmUser {
                online_id: Some(42),
                username: Some("Mapper".into()),
                country_code: Some("JP".into()),
            }),
            source: Some("Album".into()),
            tags: Some("tag1 tag2".into()),
            user_tags: vec!["first".into(), "second".into()],
            preview_time: Some(12345),
            audio_file: Some("audio.mp3".into()),
            background_file: Some("background.jpg".into()),
        }
    }

    #[test]
    fn metadata_hash_has_a_stable_versioned_encoding() {
        assert_eq!(
            metadata_hash(&fixture()).unwrap(),
            "d0ed39eaa01ceeaf916423e6bd763a88f046b0b968b7ce5ff78fafa1de8f1afe"
        );
        assert_eq!(
            metadata_hash(&fixture()).unwrap(),
            metadata_hash(&fixture()).unwrap()
        );
    }

    #[test]
    fn every_metadata_field_affects_identity() {
        let original = fixture();
        let original_hash = metadata_hash(&original).unwrap();
        let mutations: &[fn(&mut ImportedMetadata)] = &[
            |m| m.title = None,
            |m| m.title_unicode = None,
            |m| m.artist = None,
            |m| m.artist_unicode = None,
            |m| m.author = None,
            |m| m.author.as_mut().unwrap().online_id = None,
            |m| m.author.as_mut().unwrap().username = None,
            |m| m.author.as_mut().unwrap().country_code = None,
            |m| m.source = None,
            |m| m.tags = None,
            |m| m.user_tags.clear(),
            |m| m.user_tags.reverse(),
            |m| m.preview_time = None,
            |m| m.audio_file = None,
            |m| m.background_file = None,
        ];
        for mutate in mutations {
            let mut changed = original.clone();
            mutate(&mut changed);
            assert_ne!(metadata_hash(&changed).unwrap(), original_hash);
        }
    }

    #[test]
    fn metadata_hash_preserves_null_empty_and_author_presence() {
        let mutations: &[fn(&mut ImportedMetadata) -> &mut Option<String>] = &[
            |m| &mut m.title,
            |m| &mut m.title_unicode,
            |m| &mut m.artist,
            |m| &mut m.artist_unicode,
            |m| &mut m.source,
            |m| &mut m.tags,
            |m| &mut m.audio_file,
            |m| &mut m.background_file,
            |m| &mut m.author.as_mut().unwrap().username,
            |m| &mut m.author.as_mut().unwrap().country_code,
        ];
        for field in mutations {
            let mut absent = fixture();
            *field(&mut absent) = None;
            let mut empty = fixture();
            *field(&mut empty) = Some(String::new());
            assert_ne!(
                metadata_hash(&absent).unwrap(),
                metadata_hash(&empty).unwrap()
            );
        }
        let mut absent = fixture();
        absent.author = None;
        let mut present = absent.clone();
        present.author = Some(RealmUser {
            online_id: None,
            username: None,
            country_code: None,
        });
        assert_ne!(
            metadata_hash(&absent).unwrap(),
            metadata_hash(&present).unwrap()
        );
        absent.user_tags.clear();
        present = absent.clone();
        present.user_tags.push(String::new());
        assert_ne!(
            metadata_hash(&absent).unwrap(),
            metadata_hash(&present).unwrap()
        );
    }
}
