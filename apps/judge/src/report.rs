//! What a student is told about a visible case as soon as it ends (ADR-0030, ADR-0042)

use crate::{Outcome, Verdict};

const MAX_SHOWN: usize = 64 * 1024;

#[derive(Debug)]
pub struct CaseReport {
    pub position: i32,
    pub verdict: Verdict,
    pub actual: String,
    pub truncated: bool,
}

impl CaseReport {
    pub fn new(position: i32, verdict: Verdict, outcome: &Outcome) -> Self {
        let (actual, truncated) = match outcome {
            Outcome::Exited { stdout, .. } => shown(stdout),
            Outcome::TimedOut => (String::new(), false),
        };
        Self {
            position,
            verdict,
            actual,
            truncated,
        }
    }
}

// PostgreSQL `text` refuses NULL and invalid UTF-8 and a database error stops the judge:
// Without this, a program printing `\0` could stop it
fn shown(output: &[u8]) -> (String, bool) {
    let kept = &output[..output.len().min(MAX_SHOWN)];
    let mut text = String::from_utf8_lossy(kept).replace('\0', "\u{FFFD}");
    let end = text.floor_char_boundary(MAX_SHOWN);
    let truncated = output.len() > MAX_SHOWN || end < text.len();
    text.truncate(end);
    (text, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_output_is_kept_as_it_is() {
        assert_eq!(shown(b"3\n"), ("3\n".to_string(), false));
    }

    #[test]
    fn nul_and_invalid_utf8_are_replaced() {
        assert_eq!(
            shown(b"a\0b\xFFc"),
            ("a\u{FFFD}b\u{FFFD}c".to_string(), false)
        );
    }

    #[test]
    fn output_at_limit_is_kept() {
        let (text, truncated) = shown(&vec![b'a'; MAX_SHOWN]);
        assert_eq!(text.len(), MAX_SHOWN);
        assert!(!truncated);
    }

    #[test]
    fn a_long_output_is_cut_and_says_so() {
        let (text, truncated) = shown(&vec![b'a'; 16 * 1024 * 1024]);
        assert_eq!(text.len(), MAX_SHOWN);
        assert!(truncated);
    }

    #[test]
    fn replacements_stay_within_limit() {
        let (text, truncated) = shown(&vec![0; MAX_SHOWN]);
        assert!(text.len() <= MAX_SHOWN);
        assert!(text.chars().all(|character| character == '\u{FFFD}'));
        assert!(truncated);
    }

    #[test]
    fn limit_never_splits_a_character() {
        let mut output = vec![b'a'; MAX_SHOWN - 1];
        output.extend("é".as_bytes());
        let (text, truncated) = shown(&output);
        assert!(text.len() <= MAX_SHOWN);
        assert!(truncated);
    }

    #[test]
    fn timed_out_shows_nothing() {
        let report = CaseReport::new(0, Verdict::TimedOut, &Outcome::TimedOut);
        assert_eq!(report.actual, "");
        assert!(!report.truncated);
    }
}
