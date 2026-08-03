//! Shared IO for the hooks: read the event JSON from stdin, emit a decision on stdout, append a
//! JSONL decision-log record, and format a UTC timestamp — all without ever panicking (a hook must
//! fail open, never crash the session).

use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;

/// Read all of stdin as text, substituting U+FFFD for invalid UTF-8 (parity with Node's
/// `utf8` decoder). Returns `""` on any read error.
#[must_use]
pub fn read_stdin() -> String {
    let mut buf = Vec::new();
    let _ = std::io::stdin().lock().read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

/// Parse the event JSON, defaulting to an empty object on any parse error or a non-object value
/// (parity with the Node hooks: garbled/absent input is treated as `{}` and handled by the caller).
#[must_use]
pub fn parse_event(raw: &str) -> Value {
    match serde_json::from_str::<Value>(raw) {
        Ok(v) if v.is_object() => v,
        _ => Value::Object(serde_json::Map::new()),
    }
}

/// Write a decision object as one line of JSON to stdout.
pub fn emit(obj: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(obj.to_string().as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// Append one JSONL record to a decision log. Logging must never break a hook, so every error is
/// swallowed.
pub fn append_log(log_path: &Path, record: &Value) {
    if let Some(dir) = log_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = writeln!(f, "{record}");
    }
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SS+00:00` (parity with
/// `new Date().toISOString().slice(0,19)+"+00:00"`).
#[must_use]
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_utc(secs)
}

/// Format seconds-since-epoch as a UTC ISO-8601 timestamp using Howard Hinnant's civil-from-days
/// algorithm (no external date dependency).
fn format_utc(secs: u64) -> String {
    let days = i64::try_from(secs / 86400).unwrap_or(0);
    let rem = secs % 86400;
    let (hour, min, sec) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}+00:00")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_event_defaults_to_object() {
        assert!(parse_event("not json").is_object());
        assert!(parse_event("[1,2]").is_object()); // arrays -> {}
        assert_eq!(parse_event(r#"{"a":1}"#)["a"], 1);
    }

    #[test]
    fn utc_formatting_matches_known_instants() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00+00:00");
        // 2021-01-01T00:00:00Z = 1609459200
        assert_eq!(format_utc(1_609_459_200), "2021-01-01T00:00:00+00:00");
        // 2026-08-03T07:08:09Z = 1785740889
        assert_eq!(format_utc(1_785_740_889), "2026-08-03T07:08:09+00:00");
    }
}
