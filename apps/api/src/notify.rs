//! The queue's changes, passed on to every event stream of this process (ADR-0008)
//! One LISTEN connection per process rather than one per stream: an exam opens hundreds of streams

use std::time::Duration;

use sqlx::{PgPool, postgres::PgListener};
use tokio::{sync::broadcast, time};
use uuid::Uuid;

/// A run that was created or changed state, and whose it is
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunChanged {
    pub(super) id: Uuid,
    pub(super) author: String,
}

const CAPACITY: usize = 1024;
const RETRY: Duration = Duration::from_secs(1);

pub(super) fn channel() -> broadcast::Sender<RunChanged> {
    broadcast::channel(CAPACITY).0
}

/// Passes on the queue's notifications for as long as the process runs
pub(super) async fn forward(pool: PgPool, runs: broadcast::Sender<RunChanged>) {
    loop {
        if let Err(error) = listen(&pool, &runs).await {
            eprintln!("api: runs: {error}");
        }
        time::sleep(RETRY).await;
    }
}

// Returns only on error
async fn listen(pool: &PgPool, runs: &broadcast::Sender<RunChanged>) -> sqlx::Result<()> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen("runs").await?;

    loop {
        let notification = listener.recv().await?;

        match parse(notification.payload()) {
            Some(change) => drop(runs.send(change)),
            None => eprintln!("api: runs: unreadable notification"),
        }
    }
}

// "<run id> <author>" as db/migrations/0009_run_events.sql writes it
fn parse(payload: &str) -> Option<RunChanged> {
    let (id, author) = payload.split_once(' ')?;
    Some(RunChanged {
        id: id.parse().ok()?,
        author: author.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_run_then_its_author() {
        assert_eq!(
            parse("0193a5e0-0000-7000-8000-000000000000 etudiant1"),
            Some(RunChanged {
                id: "0193a5e0-0000-7000-8000-000000000000".parse().unwrap(),
                author: "etudiant1".to_owned(),
            })
        );
    }

    #[test]
    fn an_unreadable_payload_is_none() {
        assert_eq!(parse("not-a-uuid etudiant1"), None);
        assert_eq!(parse("0193a5e0-0000-7000-8000-000000000000"), None);
    }
}
