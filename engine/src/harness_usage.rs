//! What one execution actually burned, read from the harness's own output.
//!
//! TOHSENO does not change how a harness is invoked in order to meter it. The
//! private harness log is already captured verbatim for the owner; a harness
//! that reports its own total in that output is read, and one that does not is
//! honestly recorded as unmetered rather than reported as zero.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub const HARNESS_USAGE_SCHEMA: &str = "tohseno.harness-usage/1";

/// The most private log a single usage scan will read.
const MAXIMUM_LOG_SCAN_BYTES: u64 = 512 * 1024 * 1024;
/// A harness that emits one enormous line cannot exhaust memory here.
const MAXIMUM_LINE_BYTES: usize = 64 * 1024;
/// No honest token total needs more digits than this.
const MAXIMUM_COUNT_DIGITS: usize = 20;

/// The metered cost of one execution, summed across its harness attempts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessUsage {
    pub schema: String,
    pub harness: String,
    pub total_tokens: u64,
    /// Harness invocations that reported a total. One implementation attempt
    /// plus at most one ADR 0019 repair means this is normally one or two.
    pub reported_attempts: u32,
}

/// Read the token total this harness reported for one execution.
///
/// Returns `None` when the harness does not report usage in its unattended
/// output, when the log is unavailable, or when nothing parseable was found.
pub fn read_harness_usage(harness: &str, log: &Path) -> Option<HarnessUsage> {
    let marker = usage_marker(harness)?;
    let (total_tokens, reported_attempts) = scan_totals_after(log, marker)?;
    Some(HarnessUsage {
        schema: HARNESS_USAGE_SCHEMA.into(),
        harness: harness.into(),
        total_tokens,
        reported_attempts,
    })
}

/// The line a harness prints immediately before its own token total.
///
/// Only harnesses that report usage on their normal unattended path belong
/// here. Adding a harness to this table must never mean adding an argument
/// that changes what the owner sees in the private log.
fn usage_marker(harness: &str) -> Option<&'static str> {
    match harness {
        "codex" => Some("tokens used"),
        _ => None,
    }
}

fn scan_totals_after(log: &Path, marker: &str) -> Option<(u64, u32)> {
    let metadata = std::fs::symlink_metadata(log).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    let file = File::open(log).ok()?;
    let mut reader = BufReader::new(file.take(MAXIMUM_LOG_SCAN_BYTES));
    let mut line = Vec::new();
    let mut expecting = false;
    let mut total: u64 = 0;
    let mut attempts: u32 = 0;
    loop {
        line.clear();
        if read_bounded_line(&mut reader, &mut line)? == 0 {
            break;
        }
        let Ok(text) = std::str::from_utf8(&line) else {
            expecting = false;
            continue;
        };
        let text = text.trim();
        if expecting {
            if text.is_empty() {
                continue;
            }
            if let Some(value) = parse_count(text) {
                total = total.saturating_add(value);
                attempts = attempts.saturating_add(1);
            }
            expecting = false;
            continue;
        }
        expecting = text == marker;
    }
    (attempts > 0).then_some((total, attempts))
}

/// Read one line, keeping at most `MAXIMUM_LINE_BYTES` of it, and return the
/// bytes consumed. Zero means end of file.
fn read_bounded_line<R: BufRead>(reader: &mut R, line: &mut Vec<u8>) -> Option<usize> {
    let mut consumed = 0usize;
    loop {
        let available = reader.fill_buf().ok()?;
        if available.is_empty() {
            return Some(consumed);
        }
        let (used, complete) = match available.iter().position(|byte| *byte == b'\n') {
            Some(index) => (index + 1, true),
            None => (available.len(), false),
        };
        let retained = used - usize::from(complete);
        if line.len() < MAXIMUM_LINE_BYTES {
            let take = (MAXIMUM_LINE_BYTES - line.len()).min(retained);
            line.extend_from_slice(&available[..take]);
        }
        reader.consume(used);
        consumed = consumed.saturating_add(used);
        if complete {
            return Some(consumed);
        }
    }
}

fn parse_count(text: &str) -> Option<u64> {
    let digits: String = text
        .chars()
        .filter(|character| !matches!(character, ',' | '_' | ' '))
        .collect();
    if digits.is_empty()
        || digits.len() > MAXIMUM_COUNT_DIGITS
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn log_with(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("harness.log");
        let mut file = File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        (directory, path)
    }

    #[test]
    fn repair_attempts_sum_into_one_execution_total() {
        let (_directory, path) = log_with(
            "OpenAI Codex v0.149.0\nwork\ntokens used\n141,975\nmore work\ntokens used\n109,175\n",
        );
        let usage = read_harness_usage("codex", &path).unwrap();
        assert_eq!(usage.total_tokens, 251_150);
        assert_eq!(usage.reported_attempts, 2);
        assert_eq!(usage.harness, "codex");
    }

    #[test]
    fn a_harness_that_reports_nothing_is_unmetered_rather_than_zero() {
        let (_directory, path) = log_with("Claude Code finished the implementation.\n");
        assert!(read_harness_usage("claude-code", &path).is_none());
        assert!(read_harness_usage("codex", &path).is_none());
    }

    #[test]
    fn an_unparseable_total_is_not_counted_as_an_attempt() {
        let (_directory, path) = log_with("tokens used\nquite a lot actually\ntokens used\n7\n");
        let usage = read_harness_usage("codex", &path).unwrap();
        assert_eq!(usage.total_tokens, 7);
        assert_eq!(usage.reported_attempts, 1);
    }

    #[test]
    fn a_missing_or_unsafe_log_is_not_an_error() {
        let directory = tempfile::tempdir().unwrap();
        assert!(read_harness_usage("codex", &directory.path().join("absent.log")).is_none());
        assert!(read_harness_usage("codex", directory.path()).is_none());
    }

    #[test]
    fn one_enormous_line_cannot_exhaust_memory() {
        let mut contents = String::from("tokens used\n");
        contents.push_str(&"x".repeat(MAXIMUM_LINE_BYTES * 4));
        contents.push_str("\ntokens used\n1,000\n");
        let (_directory, path) = log_with(&contents);
        let usage = read_harness_usage("codex", &path).unwrap();
        assert_eq!(usage.total_tokens, 1_000);
        assert_eq!(usage.reported_attempts, 1);
    }
}
