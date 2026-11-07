//! One visible case of a run, reported as soon as it ends (ADR-0030)

use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CaseVerdict {
    Passed,
    WrongAnswer,
    Crashed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CaseResult {
    pub run: Uuid,
    pub position: u32,
    pub verdict: CaseVerdict,
    pub actual: String,
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_run_and_position() {
        let case = CaseResult {
            run: Uuid::nil(),
            position: 2,
            verdict: CaseVerdict::WrongAnswer,
            actual: "3\n".to_owned(),
            truncated: false,
        };
        assert_eq!(
            serde_json::to_value(case).unwrap(),
            json!({
                "run": "00000000-0000-0000-0000-000000000000",
                "position": 2,
                "verdict": "wrong-answer",
                "actual": "3\n",
                "truncated": false
            })
        );
    }
}
