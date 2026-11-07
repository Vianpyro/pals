//! A program sent to be judged, and the run the queue made of it

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::Id;

/// What a student can ask for (ADR-0040). A final submission is not one: only the platform makes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum RunKind {
    Precheck,
    #[default]
    Check,
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct Submission {
    pub course: Id,
    pub exercise: Id,
    pub language_pack: Id,
    pub source: String,
    #[serde(default)]
    #[ts(as = "Option<RunKind>", optional)]
    pub kind: RunKind,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Submitted {
    pub id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn submission(kind: Option<&str>) -> serde_json::Result<Submission> {
        let mut body = json!({"course": "log200", "exercise": "sum", "language_pack": "python", "source": "print(0)"});
        if let Some(kind) = kind {
            body["kind"] = json!(kind);
        }
        serde_json::from_value(body)
    }

    #[test]
    fn a_student_asks_for_a_precheck_or_a_check() {
        assert_eq!(
            submission(Some("precheck")).unwrap().kind,
            RunKind::Precheck
        );
        assert_eq!(submission(Some("check")).unwrap().kind, RunKind::Check);
    }

    #[test]
    fn the_previous_interface_sends_no_kind_and_gets_a_check() {
        assert_eq!(submission(None).unwrap().kind, RunKind::Check);
    }

    #[test]
    fn a_student_cannot_ask_for_a_final_submission() {
        assert!(submission(Some("final")).is_err());
    }
}
