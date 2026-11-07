//! The queue's changes, passed on to every event stream of this process (ADR-0008)
//! One LISTEN connection per process rather than one per stream: an exam opens hundreds of streams

use std::time::Duration;

use sqlx::{PgPool, postgres::PgListener};
use tokio::{sync::broadcast, time};
use uuid::Uuid;

/// A run that was created or changed state, or one of its visible cases that ended, and whose it is
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Changed {
    Run {
        id: Uuid,
        author: String,
    },
    Case {
        id: Uuid,
        position: u32,
        author: String,
    },
}

impl Changed {
    pub(super) fn author(&self) -> &str {
        match self {
            Self::Run { author, .. } | Self::Case { author, .. } => author,
        }
    }
}

const CAPACITY: usize = 1024;
const RETRY: Duration = Duration::from_secs(1);

pub(super) fn channel() -> broadcast::Sender<Changed> {
    broadcast::channel(CAPACITY).0
}

/// Passes on the queue's notifications for as long as the process runs
pub(super) async fn forward(pool: PgPool, changes: broadcast::Sender<Changed>) {
    loop {
        if let Err(error) = listen(&pool, &changes).await {
            eprintln!("api: changes: {error}");
        }
        time::sleep(RETRY).await;
    }
}

async fn listen(pool: &PgPool, changes: &broadcast::Sender<Changed>) -> sqlx::Result<()> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen_all(["runs", "cases"]).await?;

    loop {
        let notification = listener.recv().await?;

        match parse(notification.channel(), notification.payload()) {
            Some(change) => drop(changes.send(change)),
            None => eprintln!("api: changes: unreadable notification"),
        }
    }
}

// "<run id> <author>" on runs, "<run id> <position> <author>" on cases,
// as `db/migrations/0009_run_events.sql` writes them
fn parse(channel: &str, payload: &str) -> Option<Changed> {
    match channel {
        "runs" => parse_run(payload),
        "cases" => parse_case(payload),
        _ => None,
    }
}

fn parse_run(payload: &str) -> Option<Changed> {
    let (id, author) = payload.split_once(' ')?;
    Some(Changed::Run {
        id: id.parse().ok()?,
        author: author.to_owned(),
    })
}

fn parse_case(payload: &str) -> Option<Changed> {
    let mut parts = payload.splitn(3, ' ');
    Some(Changed::Case {
        id: parts.next()?.parse().ok()?,
        position: parts.next()?.parse().ok()?,
        author: parts.next()?.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0193a5e0-0000-7000-8000-000000000000";

    #[test]
    fn reads_a_run() {
        assert_eq!(
            parse("runs", &format!("{ID} etudiant1")),
            Some(Changed::Run {
                id: ID.parse().unwrap(),
                author: "etudiant1".to_owned()
            })
        );
    }

    #[test]
    fn reads_a_case() {
        assert_eq!(
            parse("cases", &format!("{ID} 3 etudiant1")),
            Some(Changed::Case {
                id: ID.parse().unwrap(),
                position: 3,
                author: "etudiant1".to_owned()
            })
        );
    }

    #[test]
    fn an_unreadable_payload_is_none() {
        assert_eq!(parse("runs", "not-a-uuid etudiant1"), None);
        assert_eq!(parse("runs", ID), None);
        assert_eq!(parse("cases", &format!("{ID} -1 etudiant1")), None);
        assert_eq!(parse("cases", &format!("{ID} 3")), None);
        assert_eq!(parse("elsewhere", &format!("{ID} etudiant1")), None);
    }
}
