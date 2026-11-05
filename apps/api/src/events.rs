//! The student's runs as they change, over Server-Sent Events (ADR-0008, ADR-0030)

use std::{future, time::Duration};

use axum::{
    extract::State,
    http::{HeaderName, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use contracts::Run;
use futures_util::{Stream, StreamExt, stream};
use sqlx::PgPool;
use tokio::{sync::broadcast, time};

use super::{notify::RunChanged, runs, session::Account};

const LIFETIME: Duration = Duration::from_secs(5 * 60);
// Nginx would hold the events back to fill its buffer (ADR-0008)
const UNBUFFERED: (HeaderName, &str) = (HeaderName::from_static("x-accel-buffering"), "no");

pub(super) async fn events(
    Account(author): Account,
    State(pool): State<PgPool>,
    State(changes): State<broadcast::Sender<RunChanged>>,
) -> Result<
    (
        [(HeaderName, &'static str); 1],
        Sse<impl Stream<Item = Result<Event, axum::Error>>>,
    ),
    StatusCode,
> {
    let changes = changes.subscribe();
    let snapshot = runs::latest(&pool, &author).await?;

    let updates = received(changes)
        .filter(move |change| future::ready(change.author == author))
        .filter_map(move |change| {
            let pool = pool.clone();
            async move {
                runs::one(&pool, change.id, &change.author)
                    .await
                    .ok()
                    .flatten()
            }
        });

    let stream = stream::iter(snapshot)
        .chain(updates)
        .map(|run: Run| Event::default().event("run").json_data(run))
        .take_until(time::sleep(LIFETIME));

    Ok((
        [UNBUFFERED],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
}

fn received(receiver: broadcast::Receiver<RunChanged>) -> impl Stream<Item = RunChanged> {
    stream::unfold(receiver, |mut receiver| async move {
        let change = receiver.recv().await.ok()?;
        Some((change, receiver))
    })
}
