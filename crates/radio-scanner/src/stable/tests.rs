#![allow(clippy::inconsistent_digit_grouping)] // Database versions use YYYY_MM_DD.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::float_cmp
)]

use std::{fs, path::Path};

use radio_core::{OsuKind, OsuMarker};
use tempfile::TempDir;

use super::{import_from_stable_db, media, reader};

#[derive(Clone)]
struct Entry {
    folder: String,
    difficulty: String,
    hash: String,
    artist: String,
    title: String,
    audio: String,
    osu_file: String,
    id: i32,
    duration: i32,
    timings: Vec<(f64, f64, bool)>,
    star_tags: [u8; 4],
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            folder: "123 Artist - Song".into(),
            difficulty: "Hard".into(),
            hash: "0123456789abcdef0123456789abcdef".into(),
            artist: "Artist".into(),
            title: "Song".into(),
            audio: "audio.mp3".into(),
            osu_file: "map.osu".into(),
            id: 123,
            duration: 20_000,
            timings: vec![(500.0, 0.0, true)],
            star_tags: [0x0d; 4],
        }
    }
}

fn string(bytes: &mut Vec<u8>, value: &str) {
    if value.is_empty() {
        bytes.push(0);
        return;
    }
    bytes.push(0x0b);
    let mut length = value.len();
    loop {
        let byte = u8::try_from(length & 0x7f).unwrap();
        length >>= 7;
        bytes.push(if length == 0 { byte } else { byte | 0x80 });
        if length == 0 {
            break;
        }
    }
    bytes.extend(value.as_bytes());
}

fn integer(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend(value.to_le_bytes());
}

fn entry_bytes(version: i32, entry: &Entry) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [
        &entry.artist,
        "アーティスト",
        &entry.title,
        "曲",
        "Mapper",
        &entry.difficulty,
        &entry.audio,
        &entry.hash,
        &entry.osu_file,
    ] {
        string(&mut bytes, value);
    }
    bytes.push(4); // Ranked.
    bytes.extend([0; 6 + 8]); // Counts and modified date.
    if version < 2014_06_09 {
        bytes.extend([5; 4]);
    } else {
        for _ in 0..4 {
            bytes.extend(5_f32.to_le_bytes());
        }
    }
    bytes.extend(1.4_f64.to_le_bytes());
    if version >= 2014_06_09 {
        for tag in entry.star_tags {
            if tag == 0 {
                integer(&mut bytes, 0);
                continue;
            }
            integer(&mut bytes, 2);
            for mods in [0, 64] {
                bytes.push(0x08);
                integer(&mut bytes, mods);
                bytes.push(tag);
                if tag == 0x0c {
                    bytes.extend(4.5_f32.to_le_bytes());
                } else {
                    bytes.extend(4.5_f64.to_le_bytes());
                }
            }
        }
    }
    integer(&mut bytes, 15);
    integer(&mut bytes, entry.duration);
    integer(&mut bytes, 1234);
    integer(&mut bytes, i32::try_from(entry.timings.len()).unwrap());
    for &(length, offset, uninherited) in &entry.timings {
        bytes.extend(length.to_le_bytes());
        bytes.extend(offset.to_le_bytes());
        bytes.push(u8::from(uninherited));
    }
    integer(&mut bytes, 42);
    integer(&mut bytes, entry.id);
    integer(&mut bytes, -1);
    bytes.extend([0; 4 + 2]);
    bytes.extend(0.7_f32.to_le_bytes());
    bytes.push(0);
    string(&mut bytes, "Game");
    string(&mut bytes, "TAG two");
    bytes.extend([0; 2]);
    string(&mut bytes, "");
    bytes.push(1);
    bytes.extend([0; 8]);
    bytes.push(0);
    string(&mut bytes, &entry.folder);
    bytes.extend([0; 8 + 5]);
    if version < 2014_06_09 {
        bytes.extend([0; 2]);
    }
    integer(&mut bytes, 0);
    bytes.push(10);
    bytes
}

fn database(version: i32, sizes: bool, entries: &[Entry]) -> Vec<u8> {
    let mut bytes = Vec::new();
    integer(&mut bytes, version);
    integer(&mut bytes, i32::try_from(entries.len()).unwrap());
    bytes.push(1);
    bytes.extend([0; 8]);
    string(&mut bytes, "");
    integer(&mut bytes, i32::try_from(entries.len()).unwrap());
    for entry in entries {
        let entry = entry_bytes(version, entry);
        if sizes {
            integer(&mut bytes, i32::try_from(entry.len()).unwrap());
        }
        bytes.extend(entry);
    }
    integer(&mut bytes, 1);
    bytes
}

fn decode(bytes: &[u8]) -> anyhow::Result<Vec<reader::Record>> {
    reader::decode(bytes, Path::new("fixture/osu!.db"))
}

fn install(entries: &[Entry]) -> TempDir {
    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("osu!.db"),
        database(2025_01_08, false, entries),
    )
    .unwrap();
    temp
}

fn write_osu(root: &Path, directory: &str, entry: &Entry, contents: &str) {
    let path = root
        .join(directory)
        .join(entry.folder.replace('\\', "/"))
        .join(entry.osu_file.replace('\\', "/"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn decodes_both_sides_of_layout_transitions_and_tagged_star_widths() {
    for (version, sizes) in [
        (2014_06_08, true),
        (2014_06_09, true),
        (2019_11_05, true),
        (2019_11_06, true),
        (2019_11_06, false),
        (2019_11_07, false),
        (2025_01_07, false),
        (2025_01_08, false),
    ] {
        let entry = Entry {
            star_tags: [0x0c, 0x0d, 0x0c, 0x0d],
            ..Entry::default()
        };
        let records = decode(&database(version, sizes, &[entry.clone(), entry])).unwrap();
        assert_eq!(records.len(), 2, "version {version}, sizes {sizes}");
        assert_eq!(records[1].online_id, Some(123));
        assert_eq!(records[1].beatmap.bpm, Some(120.0));
        assert_eq!(
            records[1]
                .beatmap
                .metadata
                .as_ref()
                .unwrap()
                .tags
                .as_deref(),
            Some("TAG two")
        );
    }
}

#[test]
fn maps_unicode_long_and_empty_strings_and_unavailable_fields() {
    let entry = Entry {
        title: "曲😀".repeat(1000),
        artist: String::new(),
        ..Entry::default()
    };
    let records = decode(&database(2025_01_08, false, std::slice::from_ref(&entry))).unwrap();
    let beatmap = &records[0].beatmap;
    assert_eq!(
        beatmap.hash.as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
    let metadata = beatmap.metadata.as_ref().unwrap();
    assert_eq!(metadata.title.as_deref(), Some(entry.title.as_str()));
    assert_eq!(metadata.artist, None);
    assert_eq!(metadata.artist_unicode.as_deref(), Some("アーティスト"));
    assert_eq!(metadata.title_unicode.as_deref(), Some("曲"));
    assert_eq!(metadata.preview_time, Some(1234));
    let author = metadata.author.as_ref().unwrap();
    assert_eq!(author.username.as_deref(), Some("Mapper"));
    assert_eq!(author.online_id, None);
    assert_eq!(author.country_code, None);
    assert!(metadata.user_tags.is_empty());
    let empty_stars = Entry {
        audio: String::new(),
        star_tags: [0; 4],
        ..Entry::default()
    };
    let records = decode(&database(2025_01_08, false, &[empty_stars])).unwrap();
    assert_eq!(
        records[0].beatmap.metadata.as_ref().unwrap().audio_file,
        None
    );
    assert!(
        decode(&database(2025_01_08, false, &[]))
            .unwrap()
            .is_empty()
    );
    // A tagged empty string is also valid, independently of the 0x00 marker.
    let mut bytes = database(2025_01_08, false, &[Entry::default()]);
    bytes.splice(17..18, [0x0b, 0]);
    assert_eq!(decode(&bytes).unwrap().len(), 1);
}

#[test]
fn rejects_every_truncation_without_returning_partial_records() {
    for (version, sizes) in [
        (2014_06_08, true),
        (2014_06_09, true),
        (2019_11_06, false),
        (2025_01_08, false),
    ] {
        let bytes = database(version, sizes, &[Entry::default(), Entry::default()]);
        for end in 0..bytes.len() {
            let error = decode(&bytes[..end])
                .err()
                .expect("truncated snapshot must fail");
            let message = format!("{error:#}");
            assert!(message.contains("fixture/osu!.db"), "{message}");
            assert!(message.contains("byte offset"), "{message}");
        }
    }
}

#[test]
fn rejects_bad_lengths_counts_tags_utf8_and_booleans() {
    let original = database(2025_01_08, false, &[Entry::default()]);
    for replacement in [
        vec![0xff],
        vec![0x0b, 0xff, 0xff, 0xff, 0xff, 0xff],
        vec![0x0b, 0xff, 0xff, 0xff, 0xff, 0x07],
        vec![0x0b, 1, 0xff],
    ] {
        let mut bytes = original.clone();
        bytes.splice(22..23, replacement);
        let message = format!("{:#}", decode(&bytes).err().unwrap());
        assert!(message.contains("record 0"), "{message}");
    }
    for range in [4..8, 18..22] {
        for count in [-1_i32, i32::MAX] {
            // Folder count has no relation to byte length, but cannot be negative.
            if range.start == 4 && count > 0 {
                continue;
            }
            let mut bytes = original.clone();
            bytes[range.clone()].copy_from_slice(&count.to_le_bytes());
            assert!(decode(&bytes).is_err());
        }
    }
    let star = original
        .windows(14)
        .position(|w| w[0] == 8 && w[5] == 13 && w[6..14] == 4.5_f64.to_le_bytes())
        .unwrap();
    for (offset, replacement) in [(star, 9), (star + 5, 14), (8, 2)] {
        let mut bytes = original.clone();
        bytes[offset] = replacement;
        assert!(decode(&bytes).is_err());
    }
    let mut bytes = original.clone();
    bytes[star - 4..star].copy_from_slice(&(-1_i32).to_le_bytes());
    assert!(decode(&bytes).is_err());
    let timing = original
        .windows(16)
        .position(|w| w[..8] == 500_f64.to_le_bytes() && w[8..] == 0_f64.to_le_bytes())
        .unwrap();
    for count in [-1_i32, i32::MAX] {
        let mut bytes = original.clone();
        bytes[timing - 4..timing].copy_from_slice(&count.to_le_bytes());
        assert!(decode(&bytes).is_err());
    }
    let mut bytes = original;
    bytes[timing + 16] = 2;
    assert!(decode(&bytes).is_err());
}

#[test]
fn validates_legacy_entry_sizes_and_permissions_footer() {
    let original = database(2019_11_05, true, &[Entry::default()]);
    let size = i32::from_le_bytes(original[22..26].try_into().unwrap());
    for size in [-1, 0, size - 1, size + 1, i32::MAX] {
        let mut bytes = original.clone();
        bytes[22..26].copy_from_slice(&size.to_le_bytes());
        assert!(decode(&bytes).is_err());
    }
    for permissions in [0_i32, 1, 63, 64, -1] {
        let mut bytes = original.clone();
        let offset = bytes.len() - 4;
        bytes[offset..].copy_from_slice(&permissions.to_le_bytes());
        assert_eq!(decode(&bytes).is_ok(), (0..64).contains(&permissions));
    }
    let mut bytes = original;
    bytes.push(0);
    assert!(decode(&bytes).is_err());
}

fn bpm(timings: Vec<(f64, f64, bool)>, duration: i32) -> Option<f64> {
    decode(&database(
        2025_01_08,
        false,
        &[Entry {
            duration,
            timings,
            ..Entry::default()
        }],
    ))
    .unwrap()[0]
        .beatmap
        .bpm
}

#[test]
fn converts_and_weights_uninherited_bpm_through_the_cached_duration() {
    assert_eq!(bpm(vec![(500.0, 0.0, true)], 20_000), Some(120.0));
    // The first tempo wins through total_time; using the final timing offset would pick 120.
    assert_eq!(
        bpm(vec![(500.0, 0.0, true), (250.0, 5000.0, true)], 20_000),
        Some(240.0)
    );
    assert_eq!(
        bpm(vec![(500.0, 1000.0, true), (250.0, 10_000.0, true)], 20_000),
        Some(240.0)
    );
    // Repeated sections aggregate; inherited points do not end a tempo section.
    assert_eq!(
        bpm(
            vec![
                (500.0, 0.0, true),
                (-100.0, 1000.0, false),
                (250.0, 4000.0, true),
                (500.0, 10_000.0, true)
            ],
            14_000
        ),
        Some(120.0)
    );
    assert_eq!(
        bpm(
            vec![(250.0, 10_000.0, true), (500.0, -1000.0, true)],
            20_000
        ),
        Some(240.0)
    );
    assert_eq!(
        bpm(vec![(500.0, 0.0, true), (250.0, 30_000.0, true)], 20_000),
        Some(120.0)
    );
    assert_eq!(bpm(vec![(500.0, 0.0, true)], 0), Some(120.0));
}

#[test]
fn ignores_nonpositive_nonfinite_inherited_and_missing_timings() {
    let invalid = vec![
        (0.0, 0.0, true),
        (-100.0, 100.0, true),
        (f64::NAN, 0.0, true),
        (f64::INFINITY, 0.0, true),
        (f64::MIN_POSITIVE / 10.0, 0.0, true),
        (250.0, f64::INFINITY, true),
        (250.0, f64::NAN, true),
        (250.0, 0.0, false),
    ];
    assert_eq!(bpm(invalid.clone(), 20_000), None);
    assert_eq!(bpm(Vec::new(), 20_000), None);
    let mut timings = invalid;
    timings.push((500.0, -500.0, true));
    assert_eq!(bpm(timings, 20_000), Some(120.0));
}

#[tokio::test]
async fn dispatches_stable_and_groups_normalized_folders_in_encounter_order() {
    let entries = [
        Entry {
            folder: "nested\\set".into(),
            ..Entry::default()
        },
        Entry {
            folder: "another".into(),
            id: -1,
            ..Entry::default()
        },
        Entry {
            folder: "nested/./set".into(),
            difficulty: "Easy".into(),
            ..Entry::default()
        },
        Entry {
            folder: "offline".into(),
            id: -1,
            ..Entry::default()
        },
    ];
    let temp = install(&entries);
    for entry in &entries {
        write_osu(
            temp.path(),
            "Songs",
            entry,
            "[Events]\n0,0,\"bg, space.jpg\",0,0\n",
        );
    }
    let sets = crate::get_beatmap_sets(OsuMarker {
        kind: OsuKind::Stable,
        root_path: temp.path().to_path_buf(),
        marker_path: temp.path().join("osu!.db"),
    })
    .await
    .unwrap();
    assert_eq!(sets.len(), 3);
    assert_eq!(sets[0].source, OsuKind::Stable);
    assert_eq!(sets[0].online_id, Some(123));
    assert_eq!(sets[0].hash, None);
    assert_eq!(sets[0].beatmaps.len(), 2);
    assert_eq!(sets[0].beatmaps[1].difficulty_name.as_deref(), Some("Easy"));
    assert_eq!(sets[0].files.len(), 2);
    assert!(
        sets[0]
            .files
            .iter()
            .all(|usage| usage.file.as_ref().unwrap().hash.is_none())
    );
    let audio = temp.path().join("Songs/nested/set/audio.mp3");
    for map in &sets[0].beatmaps {
        assert_eq!(sets[0].resolved_audio_path(map), Some(audio.as_path()));
    }
    assert_eq!(
        sets[0].resolved_background_path(&sets[0].beatmaps[0]),
        Some(temp.path().join("Songs/nested/set/bg, space.jpg").as_path())
    );
    assert_eq!(sets[1].online_id, None);
    assert_eq!(sets[2].online_id, None);
}

#[tokio::test]
async fn conflicting_or_unknown_set_ids_stay_absent_without_merging_folders() {
    for ids in [[123, 124, 123], [123, -1, 123], [-1, 123, 123], [0, 0, 0]] {
        let entries = ids.map(|id| Entry {
            id,
            ..Entry::default()
        });
        let temp = install(&entries);
        write_osu(temp.path(), "Songs", &entries[0], "[Events]\n");
        let sets = import_from_stable_db(&temp.path().join("osu!.db"))
            .await
            .unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].online_id, None);
    }
    let entries = [
        Entry::default(),
        Entry {
            folder: "copy".into(),
            ..Entry::default()
        },
    ];
    let temp = install(&entries);
    for entry in &entries {
        write_osu(temp.path(), "Songs", entry, "[Events]\n");
    }
    assert_eq!(
        import_from_stable_db(&temp.path().join("osu!.db"))
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn uses_custom_relative_directory_and_normalizes_windows_media_separators() {
    let entry = Entry {
        audio: "media\\audio.mp3".into(),
        osu_file: "maps\\map.osu".into(),
        ..Entry::default()
    };
    let temp = install(std::slice::from_ref(&entry));
    fs::write(
        temp.path().join("osu!.a.cfg"),
        "\u{feff}BeatmapDirectory = Custom\\Songs\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("osu!.b.cfg"),
        "BeatmapDirectory = ./Custom/Songs/\n",
    )
    .unwrap();
    write_osu(
        temp.path(),
        "Custom/Songs",
        &entry,
        "[Events]\n0,0,\"images\\bg.jpg\",0,0\n",
    );
    let sets = import_from_stable_db(&temp.path().join("osu!.db"))
        .await
        .unwrap();
    let map = &sets[0].beatmaps[0];
    let songs = fs::canonicalize(temp.path().join("Custom/Songs")).unwrap();
    assert_eq!(
        map.metadata.as_ref().unwrap().audio_file.as_deref(),
        Some("media/audio.mp3")
    );
    assert_eq!(
        sets[0].resolved_audio_path(map),
        Some(songs.join(&entry.folder).join("media/audio.mp3").as_path())
    );
    assert_eq!(
        sets[0].resolved_background_path(map),
        Some(songs.join(&entry.folder).join("images/bg.jpg").as_path())
    );
}

#[test]
fn accepts_absolute_configuration_and_rejects_conflicting_configurations() {
    let temp = TempDir::new().unwrap();
    let songs = temp.path().join("Other Songs");
    fs::write(
        temp.path().join("unrelated.cfg"),
        "BeatmapDirectory = ignored",
    )
    .unwrap();
    fs::write(
        temp.path().join("osu!.a.cfg"),
        "#BeatmapDirectory = ignored\nBeatmapDirectory = \n",
    )
    .unwrap();
    assert_eq!(
        media::songs_directory(temp.path()).unwrap(),
        temp.path().join("Songs")
    );
    fs::write(
        temp.path().join("osu!.a.cfg"),
        format!("BeatmapDirectory = {}", songs.display()),
    )
    .unwrap();
    assert_eq!(media::songs_directory(temp.path()).unwrap(), songs);
    fs::write(
        temp.path().join("osu!.b.cfg"),
        "BeatmapDirectory = conflicting",
    )
    .unwrap();
    assert!(
        media::songs_directory(temp.path())
            .unwrap_err()
            .to_string()
            .contains("conflicting BeatmapDirectory")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn resolves_windows_drive_paths_through_the_nearest_wine_prefix() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().unwrap();
    let prefix = temp.path().join("prefix");
    let installation = prefix.join("drive_c/osu!");
    let songs = temp.path().join("external/Custom Songs");
    fs::create_dir_all(&installation).unwrap();
    fs::create_dir_all(&songs).unwrap();
    fs::create_dir_all(prefix.join("dosdevices")).unwrap();
    symlink("../drive_c", prefix.join("dosdevices/c:")).unwrap();
    symlink(temp.path().join("external"), prefix.join("dosdevices/d:")).unwrap();
    // An outer mapping must never override the nearest prefix.
    fs::create_dir_all(temp.path().join("dosdevices")).unwrap();
    symlink(temp.path(), temp.path().join("dosdevices/d:")).unwrap();
    let config = installation.join("osu!.user.cfg");
    fs::write(&config, r"BeatmapDirectory = D:\Custom Songs").unwrap();
    assert_eq!(media::songs_directory(&installation).unwrap(), songs);
    fs::write(&config, r"BeatmapDirectory = C:\osu!\Songs").unwrap();
    assert_eq!(
        media::songs_directory(&installation).unwrap(),
        installation.join("Songs")
    );
    fs::write(&config, r"BeatmapDirectory = E:\Songs").unwrap();
    assert!(
        format!("{:#}", media::songs_directory(&installation).unwrap_err())
            .contains("unresolvable Wine drive")
    );
    fs::write(
        temp.path().join("osu!.user.cfg"),
        r"BeatmapDirectory = C:Songs",
    )
    .unwrap();
    assert!(media::songs_directory(temp.path()).is_err());
    let outside = TempDir::new().unwrap();
    fs::write(
        outside.path().join("osu!.user.cfg"),
        r"BeatmapDirectory = C:\Songs",
    )
    .unwrap();
    assert!(
        format!("{:#}", media::songs_directory(outside.path()).unwrap_err())
            .contains("no enclosing Wine prefix")
    );
}

#[test]
fn reads_only_background_events_with_quoted_commas_and_optional_offsets() {
    for (line, expected) in [
        ("0,0,\"bg, image.jpg\",0,0", "bg, image.jpg"),
        ("0,0,bg.jpg", "bg.jpg"),
        ("Background,0,\"image.jpg\"", "image.jpg"),
        ("0,0,\"bg \"\"quoted\"\".jpg\",0,0", "bg \"quoted\".jpg"),
    ] {
        let text = format!(
            "\u{feff}osu file format v14\n[General]\n0,0,wrong.jpg\n[Events]\n//Background\nVideo,0,video.mp4\n{line}\n[TimingPoints]\n0,0,wrong.jpg\n"
        );
        assert_eq!(
            media::background_event(&text).unwrap().as_deref(),
            Some(expected)
        );
    }
    assert_eq!(
        media::background_event("[Events]\nVideo,0,video.mp4\n").unwrap(),
        None
    );
    assert!(media::background_event("[Events]\n0,0,\"missing quote.jpg").is_err());
    assert!(media::background_event("[Events]\n0,0,\"bg.jpg\"garbage").is_err());
}

#[tokio::test]
async fn missing_unreadable_or_malformed_osu_retains_metadata_and_audio() {
    let entry = Entry::default();
    let temp = install(std::slice::from_ref(&entry));
    let path = temp
        .path()
        .join("Songs")
        .join(&entry.folder)
        .join(&entry.osu_file);
    for contents in [
        None,
        Some(vec![0xff]),
        Some(b"[Events]\n0,0,\"missing quote.jpg".to_vec()),
    ] {
        if let Some(contents) = contents {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }
        let sets = import_from_stable_db(&temp.path().join("osu!.db"))
            .await
            .unwrap();
        let map = &sets[0].beatmaps[0];
        assert_eq!(
            map.metadata.as_ref().unwrap().title.as_deref(),
            Some("Song")
        );
        assert!(sets[0].resolved_audio_path(map).is_some());
        assert_eq!(sets[0].resolved_background_path(map), None);
    }
}

#[tokio::test]
async fn rejects_absolute_and_escaping_folder_audio_osu_and_artwork_references() {
    for value in [
        "../escape",
        r"..\escape",
        "/absolute",
        r"C:\absolute",
        r"\rooted",
        r"\\server\share",
        "audio:stream",
        "bad\0path",
    ] {
        for field in 0..4 {
            let mut entry = Entry::default();
            match field {
                0 => entry.folder = value.into(),
                1 => entry.audio = value.into(),
                2 => entry.osu_file = value.into(),
                _ => {}
            }
            let temp = install(std::slice::from_ref(&entry));
            if field == 3 {
                write_osu(
                    temp.path(),
                    "Songs",
                    &entry,
                    &format!("[Events]\n0,0,\"{value}\",0,0"),
                );
            }
            let error = import_from_stable_db(&temp.path().join("osu!.db"))
                .await
                .unwrap_err();
            let message = format!("{error:#}");
            assert!(
                message.contains("record 0") && message.contains("byte offset"),
                "{message}"
            );
        }
    }
    assert_eq!(
        media::relative_path("media/.././audio.mp3").unwrap(),
        "audio.mp3"
    );
    assert!(media::relative_path(".").is_err());
}

#[tokio::test]
async fn missing_database_errors_identify_the_marker() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("osu!.db");
    assert!(
        import_from_stable_db(&path)
            .await
            .unwrap_err()
            .to_string()
            .contains(&path.display().to_string())
    );
}

fn collection_database(collections: &[(&str, &[&str])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    integer(&mut bytes, 2025_01_08);
    integer(&mut bytes, i32::try_from(collections.len()).unwrap());
    for &(name, hashes) in collections {
        string(&mut bytes, name);
        integer(&mut bytes, i32::try_from(hashes.len()).unwrap());
        for hash in hashes {
            string(&mut bytes, hash);
        }
    }
    bytes
}

#[tokio::test]
async fn imports_collections_including_empty_duplicates_unicode_and_unknown_maps() {
    let temp = install(&[
        Entry {
            hash: "0123456789ABCDEF0123456789ABCDEF".into(),
            ..Entry::default()
        },
        Entry {
            difficulty: "Easy".into(),
            hash: "123456789ABCDEF0123456789ABCDEF0".into(),
            ..Entry::default()
        },
    ]);
    let path = temp.path().join("osu!.db");
    assert!(
        super::import_snapshot_from_stable_db(&path)
            .await
            .unwrap()
            .collections
            .is_empty()
    );
    let bytes = collection_database(&[
        (
            "好きな曲 🎵",
            &[
                "ABCDEF0123456789ABCDEF0123456789",
                "ffffffffffffffffffffffffffffffff",
                "ABCDEF0123456789ABCDEF0123456789",
            ],
        ),
        ("好きな曲 🎵", &[]),
        ("", &[]),
    ]);
    fs::write(temp.path().join("collection.db"), &bytes).unwrap();
    let snapshot = crate::import_snapshot(OsuMarker {
        kind: OsuKind::Stable,
        marker_path: path.clone(),
        root_path: temp.path().to_owned(),
    })
    .await
    .unwrap();
    assert_eq!(snapshot.beatmap_sets[0].beatmaps.len(), 2);
    assert_eq!(
        snapshot.beatmap_sets[0].beatmaps[0].md5_hash.as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
    assert_eq!(
        snapshot.beatmap_sets[0].beatmaps[0].hash.as_deref(),
        Some("0123456789ABCDEF0123456789ABCDEF")
    );
    assert_eq!(snapshot.collections.len(), 3);
    assert_eq!(snapshot.collections[0].source_id, "[\"好きな曲 🎵\",0]");
    assert_eq!(snapshot.collections[1].source_id, "[\"好きな曲 🎵\",1]");
    assert_eq!(snapshot.collections[2].source_id, "[\"\",0]");
    assert!(snapshot.collections[1].beatmap_md5_hashes.is_empty());
    assert_eq!(
        snapshot.collections[0].beatmap_md5_hashes,
        vec![
            "abcdef0123456789abcdef0123456789",
            "ffffffffffffffffffffffffffffffff",
            "abcdef0123456789abcdef0123456789"
        ]
    );
    assert_eq!(fs::read(temp.path().join("collection.db")).unwrap(), bytes);
    assert_eq!(import_from_stable_db(&path).await.unwrap().len(), 1);
    fs::write(temp.path().join("collection.db"), &bytes[..bytes.len() - 1]).unwrap();
    assert!(
        import_from_stable_db(&path).await.is_err(),
        "legacy wrapper must fully parse collections"
    );
}

#[test]
fn rejects_corrupt_collection_databases_without_partial_results() {
    let path = Path::new("fixture/collection.db");
    let valid = collection_database(&[
        ("test", &["0123456789abcdef0123456789abcdef"]),
        ("empty", &[]),
    ]);
    for length in 0..valid.len() {
        let error = reader::decode_collections(&valid[..length], path).unwrap_err();
        assert!(format!("{error:#}").contains("collection.db"));
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(reader::decode_collections(&trailing, path).is_err());
    for hash in [
        "",
        "0123",
        "x123456789abcdef0123456789abcdef",
        "é123456789abcdef0123456789abcde",
    ] {
        assert!(
            reader::decode_collections(&collection_database(&[("test", &[hash])]), path).is_err()
        );
    }
    let mut negative = valid.clone();
    negative[4..8].copy_from_slice(&(-1_i32).to_le_bytes());
    assert!(reader::decode_collections(&negative, path).is_err());
    let mut malformed_name = valid;
    malformed_name[8] = 0xff;
    assert!(reader::decode_collections(&malformed_name, path).is_err());
}
