use radio_db::repositories::beatmap_set::{
    Comparison, DateCriterion, DateField, NumericCriterion, NumericField, NumericValue,
    TextCriterion, TextField,
};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug)]
pub(crate) struct Condition {
    pub criterion: TextCriterion,
    pub negative: bool,
    pub phrase: bool,
}

pub(crate) struct Search {
    pub conditions: Vec<Condition>,
    pub numeric: Vec<NumericCriterion>,
    pub statuses: Vec<(Vec<String>, bool)>,
    pub dates: Vec<DateCriterion>,
    pub played: Vec<bool>,
    pub ages: Vec<(Comparison, i64, bool)>,
    pub online_id: Option<i32>,
    pub numeric_condition: Option<usize>,
}

// All slice offsets come from char_indices or from an ASCII operator found in the same string.
#[allow(
    clippy::string_slice,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines
)]
pub(crate) fn parse(query: &str) -> Search {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut quoted = false;
    for (i, c) in query.char_indices() {
        if c.is_whitespace() && !quoted {
            if let Some(start) = start.take() {
                tokens.push(&query[start..i]);
            }
        } else {
            start.get_or_insert(i);
            if c == '"' {
                quoted = !quoted;
            }
        }
    }
    if let Some(start) = start {
        if quoted {
            tokens.extend(query[start..].split_whitespace());
        } else {
            tokens.push(&query[start..]);
        }
    }
    let mut conditions = Vec::new();
    let mut numeric = Vec::new();
    let mut statuses = Vec::new();
    let mut dates = Vec::new();
    let mut played = Vec::new();
    let mut ages = Vec::new();
    let mut free = Vec::new();
    for token in tokens {
        if let Some((name, op, value)) = parts(token) {
            let recognized = match name.to_ascii_lowercase().as_str() {
                "status" if matches!(op, Comparison::Equal | Comparison::NotEqual) => {
                    let values: Vec<_> = value.split(',').map(str::to_ascii_lowercase).collect();
                    if values.iter().all(|v| {
                        v == "none" || v.parse::<radio_core::import_types::BeatmapStatus>().is_ok()
                    }) {
                        statuses.push((values, op == Comparison::NotEqual));
                        true
                    } else {
                        false
                    }
                }
                "ranked" | "submitted" | "created" => {
                    if let Some((start, end)) = date_period(value) {
                        dates.push(DateCriterion {
                            field: if name.eq_ignore_ascii_case("ranked") {
                                DateField::Ranked
                            } else {
                                DateField::Submitted
                            },
                            comparison: op,
                            start,
                            end,
                        });
                        true
                    } else {
                        false
                    }
                }
                "played" if matches!(op, Comparison::Equal | Comparison::NotEqual) => {
                    let value = match value.to_ascii_lowercase().as_str() {
                        "true" | "yes" | "1" => Some(true),
                        "false" | "no" | "0" => Some(false),
                        _ => None,
                    };
                    value.is_some_and(|value| {
                        played.push(value ^ (op == Comparison::NotEqual));
                        true
                    })
                }
                "lastplayed" => {
                    if let Some((whole, fractional)) = parse_duration(value, true) {
                        ages.push((op, whole, fractional));
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if recognized {
                continue;
            }
        }
        if let Some(criterion) = parse_numeric(token) {
            numeric.push(criterion);
            continue;
        }
        let field = token.find(['=', ':']).and_then(|pos| {
            let (name, op) = token[..pos]
                .strip_suffix('!')
                .map_or((&token[..pos], false), |name| (name, true));
            let field = match name.to_ascii_lowercase().as_str() {
                "artist" => TextField::Artist,
                "title" => TextField::Title,
                "creator" | "author" | "mapper" => TextField::Mapper,
                "diff" => TextField::Difficulty,
                "source" => TextField::Source,
                "tag" => TextField::Tag,
                _ => return None,
            };
            Some((field, op, &token[pos + 1..]))
        });
        let parsed = field.and_then(|(field, negative, value)| {
            value_parts(value).map(|(value, phrase, exact)| (field, negative, value, phrase, exact))
        });
        if parsed.is_none()
            && value_parts(token).is_none()
            && token.chars().any(char::is_whitespace)
        {
            for part in token.split_whitespace() {
                free.push((conditions.len(), part));
                conditions.push(Condition {
                    criterion: TextCriterion {
                        field: TextField::Any,
                        value: part.to_lowercase(),
                        exact: false,
                    },
                    negative: false,
                    phrase: false,
                });
            }
            continue;
        }
        let (field, negative, value, phrase, exact) = parsed.unwrap_or_else(|| {
            free.push((conditions.len(), token));
            let (value, phrase, exact) = value_parts(token).unwrap_or((token, false, false));
            (TextField::Any, false, value, phrase, exact)
        });
        conditions.push(Condition {
            criterion: TextCriterion {
                field,
                value: value.to_lowercase(),
                exact,
            },
            negative,
            phrase,
        });
    }
    let online = if let [(index, token)] = free.as_slice() {
        if token.bytes().all(|b| b.is_ascii_digit()) {
            token
                .parse::<i32>()
                .ok()
                .filter(|n| *n > 0)
                .map(|id| (*index, id))
        } else {
            None
        }
    } else {
        None
    };
    Search {
        conditions,
        numeric,
        statuses,
        dates,
        played,
        ages,
        online_id: online.map(|(_, id)| id),
        numeric_condition: online.map(|(index, _)| index),
    }
}

fn parse_numeric(token: &str) -> Option<NumericCriterion> {
    let (name, comparison, value) = parts(token)?;
    let field = match name.to_ascii_lowercase().as_str() {
        "bpm" => NumericField::Bpm,
        "star" | "stars" | "sr" => NumericField::Stars,
        "length" => NumericField::Length,
        _ => return None,
    };
    let value = if field == NumericField::Length {
        let (whole, fractional) = parse_length(value)?;
        NumericValue::Milliseconds { whole, fractional }
    } else {
        let number = value.parse::<f64>().ok()?;
        if !number.is_finite() {
            return None;
        }
        NumericValue::Number(number)
    };
    Some(NumericCriterion {
        field,
        comparison,
        value,
    })
}

fn parts(token: &str) -> Option<(&str, Comparison, &str)> {
    let pos = token.find(['=', '!', ':', '<', '>'])?;
    let (name, suffix) = token.split_at(pos);
    [
        ("!=", Comparison::NotEqual),
        ("!:", Comparison::NotEqual),
        ("<=", Comparison::LessEqual),
        ("<:", Comparison::LessEqual),
        (">=", Comparison::GreaterEqual),
        (">:", Comparison::GreaterEqual),
        ("=", Comparison::Equal),
        (":", Comparison::Equal),
        ("<", Comparison::Less),
        (">", Comparison::Greater),
    ]
    .into_iter()
    .find_map(|(prefix, op)| suffix.strip_prefix(prefix).map(|v| (name, op, v)))
}

#[allow(clippy::indexing_slicing)] // Lengths are checked before indexing.
fn date_period(value: &str) -> Option<(i64, i64)> {
    use chrono::NaiveDate;
    let pieces: Vec<_> = value.split(['-', '/', '.']).collect();
    if pieces.is_empty()
        || pieces.len() > 3
        || pieces[0].len() != 4
        || pieces
            .iter()
            .any(|p| !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    if pieces.len() > 1 && (pieces[1].len() != 2) {
        return None;
    }
    if pieces.len() > 2
        && (pieces[2].len() != 2 || value.chars().nth(7)? != value.chars().nth(4)?)
    {
        return None;
    }
    let year: i32 = pieces[0].parse().ok()?;
    let month: u32 = pieces.get(1).map_or(Some(1), |v| v.parse().ok())?;
    let day = pieces.get(2).map_or(Some(1), |v| v.parse().ok())?;
    let start = NaiveDate::from_ymd_opt(year, month, day)?;
    let end = match pieces.len() {
        1 => NaiveDate::from_ymd_opt(year.checked_add(1)?, 1, 1)?,
        2 if month == 12 => NaiveDate::from_ymd_opt(year.checked_add(1)?, 1, 1)?,
        2 => NaiveDate::from_ymd_opt(year, month.checked_add(1)?, 1)?,
        _ => start.succ_opt()?,
    };
    Some((
        start.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis(),
        end.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis(),
    ))
}

// Parse duration as exact decimal milliseconds; sub-millisecond comparisons retain their remainder.
fn parse_length(value: &str) -> Option<(i64, bool)> {
    parse_duration(value, false)
}

fn parse_duration(value: &str, age: bool) -> Option<(i64, bool)> {
    let value = value.to_ascii_lowercase();
    let (value, unit) = [
        ("ms", 1_i128),
        ("w", 604_800_000),
        ("d", 86_400_000),
        ("s", 1_000),
        ("m", 60_000),
        ("h", 3_600_000),
    ]
    .into_iter()
    .find_map(|(suffix, unit)| value.strip_suffix(suffix).map(|number| (number, unit)))
    .unwrap_or((value.as_str(), if age { 86_400_000 } else { 1_000 }));
    if (age && (value.contains(':') || unit == 1)) || (!age && unit >= 86_400_000) {
        return None;
    }
    let parts: Vec<_> = value.split(':').collect();
    let (prefix, last) = match parts.as_slice() {
        [last] => (0_i128, *last),
        [minutes, seconds] if unit == 1_000 => {
            if seconds.split('.').next()?.len() != 2 {
                return None;
            }
            (parse_integer(minutes)?.checked_mul(60_000)?, *seconds)
        }
        [hours, minutes, seconds] if unit == 1_000 => {
            if minutes.len() != 2 || seconds.split('.').next()?.len() != 2 {
                return None;
            }
            let minutes = parse_integer(minutes)?;
            if minutes >= 60 {
                return None;
            }
            (
                parse_integer(hours)?
                    .checked_mul(3_600_000)?
                    .checked_add(minutes.checked_mul(60_000)?)?,
                *seconds,
            )
        }
        _ => return None,
    };
    let (integer, fraction) = last
        .split_once('.')
        .map_or((last, None), |(i, f)| (i, Some(f)));
    let integer = parse_integer(integer)?;
    if parts.len() > 1 && integer >= 60 {
        return None;
    }
    let mut whole = prefix.checked_add(integer.checked_mul(unit)?)?;
    let mut fractional = false;
    if let Some(fraction) = fraction {
        let fraction = fraction.trim_end_matches('0');
        if fraction.is_empty() {
            if last.ends_with('.') {
                return None;
            }
        } else {
            let numerator = parse_integer(fraction)?.checked_mul(unit)?;
            let denominator = 10_i128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
            whole = whole.checked_add(numerator.checked_div(denominator)?)?;
            fractional = numerator.checked_rem(denominator)? != 0;
        }
    }
    Some((i64::try_from(whole).ok()?, fractional))
}

fn parse_integer(value: &str) -> Option<i128> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn value_parts(value: &str) -> Option<(&str, bool, bool)> {
    if value.is_empty() || value.starts_with(['=', ':', '<', '>', '!']) {
        return None;
    }
    if let Some(quoted) = value.strip_prefix('"') {
        let (quoted, exact) = quoted
            .strip_suffix('!')
            .map_or((quoted, false), |s| (s, true));
        let inner = quoted.strip_suffix('"')?;
        if inner.is_empty() || inner.contains('"') {
            return None;
        }
        Some((inner, !exact, exact))
    } else if value.contains('"') {
        None
    } else {
        Some((value, false, false))
    }
}

// .NET's boundary word set differs from Rust's Unicode \b (notably combining marks).
#[allow(clippy::expect_used)] // The regex is a fixed, tested Unicode character class.
fn word(c: char) -> bool {
    static WORD: OnceLock<Regex> = OnceLock::new();
    WORD.get_or_init(|| {
        Regex::new(r"\A[\p{L}\p{Mn}\p{Nd}\p{Pc}\u{200C}\u{200D}]\z").expect("valid word class")
    })
    .is_match(c.encode_utf8(&mut [0; 4]))
}

// Start is a character boundary; starts_with ensures the end is also within the string.
#[allow(clippy::string_slice, clippy::arithmetic_side_effects)]
pub(crate) fn phrase_matches(text: &str, phrase: &str) -> bool {
    text.char_indices().any(|(start, first)| {
        if !text[start..].starts_with(phrase) {
            return false;
        }
        let end = start + phrase.len();
        let before = text[..start].chars().next_back().is_some_and(word);
        let after = text[end..].chars().next().is_some_and(word);
        before != word(first) && phrase.chars().next_back().is_some_and(word) != after
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_parsing_preserves_duration_precision_and_rejects_invalid_values() {
        for input in ["61.001", "1:01.001", "0:01:01.001", "61001ms", "61.001s"] {
            assert_eq!(parse_length(input), Some((61_001, false)), "{input}");
        }
        for input in ["1.0001s", "1000.1ms"] {
            assert_eq!(parse_length(input), Some((1_000, true)));
        }
        assert_eq!(parse_length("1m"), Some((60_000, false)));
        assert_eq!(parse_length("1h"), Some((3_600_000, false)));
        for input in [
            "",
            "-1",
            "1:60",
            "1:2",
            "1:60:00",
            "1.",
            "nan",
            "99999999999999999999999h",
            "1:02:03:04",
        ] {
            assert_eq!(parse_length(input), None, "{input}");
        }
        for input in [
            "bpm=nan",
            "stars=inf",
            "length=1:60",
            "bpm==10",
            "sr<=",
            "length=99999999999999999999999h",
        ] {
            let parsed = parse(input);
            assert!(parsed.numeric.is_empty(), "{input}");
            assert_eq!(parsed.conditions[0].criterion.value, input);
        }
        let parsed = parse("bpm>=180 star<:6 sr!:5 length=1:01.001");
        assert_eq!(parsed.numeric.len(), 4);
        assert!(parsed.conditions.is_empty());
    }

    #[test]
    fn temporal_parsing_rejects_invalid_fragments() {
        for input in [
            "ranked=2023-02-29",
            "ranked=2024/13",
            "ranked=2024-02/29",
            "status=ranked,unknown",
            "status>ranked",
            "played=maybe",
            "lastplayed=1ms",
            "lastplayed=-1d",
        ] {
            let search = parse(input);
            assert!(
                search.statuses.is_empty()
                    && search.dates.is_empty()
                    && search.played.is_empty()
                    && search.ages.is_empty(),
                "{input}"
            );
            assert_eq!(search.conditions[0].criterion.value, input);
        }
        assert!(date_period("2024-02-29").is_some());
        assert!(date_period("2024/02/29").is_some());
        assert!(date_period("2024.02.29").is_some());
    }

    #[test]
    fn parsing_and_dotnet_boundaries() {
        let search =
            parse("artist=xi artist!=\"xi\"! unknown=xx bpm>=nope title= \"unfinished phrase");
        assert_eq!(search.conditions.len(), 7);
        assert!(search.conditions[1].negative && search.conditions[1].criterion.exact);
        assert_eq!(search.conditions[2].criterion.value, "unknown=xx");
        assert_eq!(search.conditions[3].criterion.value, "bpm>=nope");
        assert_eq!(search.conditions[5].criterion.value, "\"unfinished");
        let unknown = parse("unknown=\"one two\"");
        assert_eq!(
            unknown
                .conditions
                .iter()
                .map(|c| c.criterion.value.as_str())
                .collect::<Vec<_>>(),
            ["unknown=\"one", "two\""]
        );
        assert_eq!(parse("42 artist=xi").online_id, Some(42));
        assert_eq!(parse("0").online_id, None);
        assert_eq!(parse("42 words").online_id, None);
        assert!(phrase_matches("(ёж) xi!", "ёж"));
        assert!(phrase_matches("xi", "xi"));
        assert!(!phrase_matches("xion", "xi"));
        assert!(phrase_matches("xi\u{0903}", "xi")); // Mc: Rust \b treats this as word; .NET does not.
        assert!(!phrase_matches("xi\u{0301}", "xi")); // Mn is a .NET word character.
    }
}
