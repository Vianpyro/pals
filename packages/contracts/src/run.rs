//! A run as its author sees it, live or in their history (ADR-0030)

use crate::RunKind;
use serde::Serialize;
use time::OffsetDateTime;
use ts_rs::TS;
use uuid::Uuid;

/// Where a run is, and its outcome once judged. Only a judged run carries `passed`:
/// a waiting run with a result cannot be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "lowercase")]
#[ts(export)]
pub enum RunStatus {
    Waiting,
    Running,
    Done { passed: bool },
    Replaced,
    Failed,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Run {
    pub id: Uuid,
    pub kind: RunKind,
    #[serde(flatten)]
    pub status: RunStatus,
    #[serde(with = "time::serde::rfc3339")]
    #[ts(type = "string")]
    pub enqueued_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use time::macros::datetime;

    #[test]
    fn only_a_judged_run_says_whether_it_passed() {
        assert_eq!(
            serde_json::to_value(RunStatus::Done { passed: true }).unwrap(),
            json!({"state": "done", "passed": true})
        );
        assert_eq!(
            serde_json::to_value(RunStatus::Waiting).unwrap(),
            json!({"state": "waiting"})
        );
    }

    #[test]
    fn a_run_carries_its_status_beside_its_id() {
        let run = Run {
            id: Uuid::nil(),
            kind: RunKind::Precheck,
            status: RunStatus::Done { passed: false },
            enqueued_at: datetime!(2026-01-01 09:00 UTC),
        };
        assert_eq!(
            serde_json::to_value(run).unwrap(),
            json!({
                "id": "00000000-0000-0000-0000-000000000000",
                "kind": "precheck",
                "state": "done",
                "passed": false,
                "enqueued_at": "2026-01-01T09:00:00Z"
            })
        );
    }
}
