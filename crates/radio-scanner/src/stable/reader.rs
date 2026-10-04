#![allow(clippy::inconsistent_digit_grouping)] // Database versions use YYYY_MM_DD.

use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result, ensure};
use radio_core::import_types::{BeatmapMetadata, ImportedBeatmap, ImportedCollection, RealmUser};

pub(super) struct Record {
    pub index: usize,
    pub offset: usize,
    pub folder: String,
    pub osu_file: String,
    pub online_id: Option<i32>,
    pub beatmap: ImportedBeatmap,
}

pub(super) fn decode(bytes: &[u8], path: &Path) -> Result<Vec<Record>> {
    let version = Reader::new(bytes, path).i32()?;
    if version == 2019_11_06 {
        // The wiki and stable readers disagree on this boundary. Validate the
        // entire snapshot before accepting either layout (no filesystem work).
        match decode_layout(bytes, path, false) {
            Ok(records) => Ok(records),
            Err(without_size) => decode_layout(bytes, path, true).with_context(|| {
                format!("2019_11_06 also failed without entry sizes: {without_size:#}")
            }),
        }
    } else {
        decode_layout(bytes, path, version < 2019_11_06)
    }
}

pub(super) fn decode_collections(bytes: &[u8], path: &Path) -> Result<Vec<ImportedCollection>> {
    let mut reader = Reader::new(bytes, path);
    let version = reader.i32()?;
    reader.check(version > 0, "invalid collection database version")?;
    let count = reader.count("collection count", 5)?;
    let mut occurrences = HashMap::<String, usize>::new();
    let mut collections = Vec::new();
    for index in 0..count {
        reader.record = Some(index);
        let name = reader.string()?.unwrap_or_default();
        let occurrence = occurrences.entry(name.clone()).or_default();
        let source_id = serde_json::to_string(&(&name, *occurrence))?;
        *occurrence = occurrence
            .checked_add(1)
            .context("collection occurrence overflow")?;
        let hash_count = reader.count("collection beatmap count", 34)?;
        let mut beatmap_md5_hashes = Vec::new();
        for _ in 0..hash_count {
            let hash = reader
                .string()?
                .with_context(|| reader.context("missing collection beatmap MD5"))?;
            beatmap_md5_hashes.push(
                crate::normalize_md5(&hash)
                    .with_context(|| reader.context("invalid collection beatmap MD5"))?,
            );
        }
        collections.push(ImportedCollection {
            source_id,
            name,
            beatmap_md5_hashes,
        });
    }
    reader.check(
        reader.position == bytes.len(),
        "trailing bytes after collections",
    )?;
    Ok(collections)
}

fn decode_layout(bytes: &[u8], path: &Path, entry_sizes: bool) -> Result<Vec<Record>> {
    let mut reader = Reader::new(bytes, path);
    let version = reader.i32()?;
    reader.check(version > 0, "invalid database version")?;
    reader.nonnegative("folder count")?;
    reader.boolean()?;
    reader.take(8)?; // Account unlock date (Windows ticks).
    reader.string()?; // Player name.
    let count = reader.count("beatmap count", 1)?;
    let mut records = Vec::new();
    for index in 0..count {
        reader.record = Some(index);
        let offset = reader.position;
        let entry_end = if entry_sizes {
            let size = reader.nonnegative("entry size")?;
            let end = reader.position.checked_add(size);
            reader.check(
                end.is_some_and(|end| end <= bytes.len()),
                "invalid entry size",
            )?;
            reader.limit = end.context("entry size overflow")?;
            Some(reader.limit)
        } else {
            None
        };
        records.push(reader.beatmap(version, index, offset)?);
        if let Some(end) = entry_end {
            reader.check(
                reader.position == end,
                "entry size does not match decoded record",
            )?;
            reader.limit = bytes.len();
        }
    }
    reader.record = None;
    let permissions = reader.i32()?;
    reader.check(
        permissions >= 0 && permissions & !63 == 0,
        "invalid permissions footer",
    )?;
    reader.check(
        reader.position == bytes.len(),
        "trailing bytes after permissions footer",
    )?;
    Ok(records)
}

struct Reader<'a> {
    bytes: &'a [u8],
    path: &'a Path,
    position: usize,
    limit: usize,
    record: Option<usize>,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8], path: &'a Path) -> Self {
        Self {
            bytes,
            path,
            position: 0,
            limit: bytes.len(),
            record: None,
        }
    }

    fn context(&self, message: &str) -> String {
        let record = self
            .record
            .map_or_else(|| "header/footer".to_owned(), |i| format!("record {i}"));
        format!(
            "{}: {record}, byte offset {}: {message}",
            self.path.display(),
            self.position
        )
    }

    fn check(&self, valid: bool, message: &str) -> Result<()> {
        ensure!(valid, "{}", self.context(message));
        Ok(())
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(length);
        self.check(
            end.is_some_and(|end| end <= self.limit),
            "truncated field or invalid length",
        )?;
        let end = end.context("byte offset overflow")?;
        let value = self
            .bytes
            .get(self.position..end)
            .with_context(|| self.context("truncated field"))?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?
            .try_into()
            .with_context(|| self.context("invalid field width"))
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    fn boolean(&mut self) -> Result<bool> {
        let value = self.byte()?;
        self.check(value <= 1, "invalid Boolean")?;
        Ok(value == 1)
    }

    fn nonnegative(&mut self, field: &str) -> Result<usize> {
        usize::try_from(self.i32()?).with_context(|| self.context(&format!("negative {field}")))
    }

    fn count(&mut self, field: &str, minimum_width: usize) -> Result<usize> {
        let count = self.nonnegative(field)?;
        let maximum = self
            .limit
            .saturating_sub(self.position)
            .checked_div(minimum_width)
            .context("zero minimum field width")?;
        self.check(count <= maximum, &format!("invalid {field}"))?;
        Ok(count)
    }

    fn string(&mut self) -> Result<Option<String>> {
        let tag = self.byte()?;
        if tag == 0 {
            return Ok(None);
        }
        self.check(tag == 0x0b, "invalid string tag")?;
        let mut length = 0_u32;
        for shift in (0..35).step_by(7) {
            let byte = self.byte()?;
            self.check(shift < 28 || byte <= 7, "overflowing ULEB128 string length")?;
            length |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                let length = usize::try_from(length)
                    .with_context(|| self.context("string length overflow"))?;
                let value = std::str::from_utf8(self.take(length)?)
                    .with_context(|| self.context("invalid UTF-8 string"))?;
                return Ok((!value.is_empty()).then(|| value.to_owned()));
            }
        }
        anyhow::bail!("{}", self.context("unterminated ULEB128 string length"))
    }

    #[allow(clippy::too_many_lines)] // Fixed positional layout from the official osu!.db specification.
    fn beatmap(&mut self, version: i32, index: usize, offset: usize) -> Result<Record> {
        let artist = self.string()?;
        let artist_unicode = self.string()?;
        let title = self.string()?;
        let title_unicode = self.string()?;
        let creator = self.string()?;
        let difficulty_name = self.string()?;
        let audio_file = self.string()?;
        let hash = self.string()?;
        let osu_file = self
            .string()?
            .with_context(|| self.context("empty .osu filename"))?;
        self.take(1 + 6 + 8)?; // Status, object counts, last modification ticks.
        self.take(if version < 2014_06_09 { 4 } else { 16 })?; // AR, CS, HP, OD.
        self.take(8)?; // Slider velocity.
        if version >= 2014_06_09 {
            for _ in 0..4 {
                let count = self.count("star rating count", 10)?;
                for _ in 0..count {
                    let tag = self.byte()?;
                    self.check(tag == 0x08, "invalid star rating integer tag")?;
                    self.take(4)?; // Mods.
                    let tag = self.byte()?;
                    let width = match tag {
                        0x0c => 4,
                        0x0d => 8,
                        _ => anyhow::bail!("{}", self.context("invalid star rating numeric tag")),
                    };
                    self.take(width)?;
                }
            }
        }
        self.take(4)?; // Drain time (seconds).
        let total_time = self.i32()?;
        let preview_time = Some(self.i32()?);
        let count = self.count("timing point count", 17)?;
        let mut timings = Vec::new();
        for _ in 0..count {
            let beat_length = self.f64()?;
            let offset = self.f64()?;
            let uninherited = self.boolean()?;
            let bpm = 60_000.0 / beat_length;
            if uninherited
                && beat_length > 0.0
                && beat_length.is_finite()
                && offset.is_finite()
                && bpm > 0.0
                && bpm.is_finite()
            {
                timings.push((offset, bpm));
            }
        }
        self.take(4)?; // Difficulty ID.
        let id = self.i32()?;
        let online_id = (id > 0).then_some(id);
        self.take(4 + 4 + 2 + 4 + 1)?; // Thread ID, grades, local offset, stack leniency, mode.
        let source = self.string()?;
        let tags = self.string()?;
        self.take(2)?; // Online offset.
        self.string()?; // Title font.
        self.boolean()?; // Unplayed.
        self.take(8)?; // Last played ticks.
        self.boolean()?; // osz2.
        let folder = self
            .string()?
            .with_context(|| self.context("empty beatmap folder"))?;
        self.take(8)?; // Last repository check ticks.
        for _ in 0..5 {
            self.boolean()?;
        } // Sound, skin, storyboard, video and visual overrides.
        if version < 2014_06_09 {
            self.take(2)?;
        }
        self.take(4 + 1)?; // Last edit time and mania scroll speed.
        Ok(Record {
            index,
            offset,
            folder,
            osu_file,
            online_id,
            beatmap: ImportedBeatmap {
                difficulty_name,
                md5_hash: hash
                    .as_deref()
                    .map(crate::normalize_md5)
                    .transpose()
                    .with_context(|| self.context("invalid beatmap MD5"))?,
                hash,
                bpm: representative_bpm(timings, total_time),
                metadata: Some(BeatmapMetadata {
                    artist,
                    artist_unicode,
                    title,
                    title_unicode,
                    author: creator.map(|username| RealmUser {
                        online_id: None,
                        username: Some(username),
                        country_code: None,
                    }),
                    source,
                    tags,
                    user_tags: Vec::new(),
                    preview_time,
                    audio_file,
                    background_file: None,
                }),
            },
        })
    }
}

fn representative_bpm(mut timings: Vec<(f64, f64)>, total_time: i32) -> Option<f64> {
    timings.sort_by(|a, b| a.0.total_cmp(&b.0));
    let end = f64::from(total_time.max(0));
    let mut durations = HashMap::<u64, f64>::new();
    for (index, &(offset, bpm)) in timings.iter().enumerate() {
        // Stable extends its first timing section back to the start of the song.
        let start = if index == 0 {
            0.0
        } else {
            offset.clamp(0.0, end)
        };
        let next = timings
            .get(index.saturating_add(1))
            .map_or(end, |point| point.0.clamp(0.0, end));
        let duration = if offset > end {
            0.0
        } else {
            (next - start).max(0.0)
        };
        *durations.entry(bpm.to_bits()).or_default() += duration;
    }
    durations
        .into_iter()
        .max_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then_with(|| f64::from_bits(a.0).total_cmp(&f64::from_bits(b.0)))
        })
        .map(|(bpm, _)| f64::from_bits(bpm))
}
