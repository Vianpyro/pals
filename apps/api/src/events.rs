//! The student's runs and their visible cases as they change, over Server-Sent Events (ADR-0008, ADR-0030)

use std::{future, time::Duration};

use axum::{
    extract::State,
    http::{HeaderName, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use contracts::{CaseResult, Run};
use futures_util::{Stream, StreamExt, stream};
use sqlx::PgPool;
use tokio::{sync::broadcast, time};

use super::{cases, notify::Changed, runs, session::Account};

const LIFETIME: Duration = Duration::from_secs(5 * 60);
// Nginx would hold the events back to fill its buffer (ADR-0008)
const UNBUFFERED: (HeaderName, &str) = (HeaderName::from_static("x-accel-buffering"), "no");

pub(super) async fn events(
    Account(author): Account,
    State(pool): State<PgPool>,
    State(changes): State<broadcast::Sender<Changed>>,
) -> Result<
    (
        [(HeaderName, &'static str); 1],
        Sse<impl Stream<Item = Result<Event, axum::Error>>>,
    ),
    StatusCode,
> {
    // Before the snapshot: a change made while it is read still arrives
    let changes = changes.subscribe();
    let snapshot = snapshot(&pool, &author).await?;

    let updates = received(changes)
        .filter(move |change| future::ready(change.author() == author))
        .filter_map(move |change| {
            let pool = pool.clone();
            async move { update(&pool, change).await }
        });

    let stream = stream::iter(snapshot)
        .chain(updates)
        .take_until(time::sleep(LIFETIME));

    Ok((
        [UNBUFFERED],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
}

async fn snapshot(
    pool: &PgPool,
    author: &str,
) -> Result<Vec<Result<Event, axum::Error>>, StatusCode> {
    let runs = runs::latest(pool, author).await?;
    let ids: Vec<_> = runs.iter().map(|run| run.id).collect();
    let cases = cases::of(pool, &ids).await?;
    Ok(runs
        .into_iter()
        .map(run_event)
        .chain(cases.into_iter().map(case_event))
        .collect())
}

async fn update(pool: &PgPool, change: Changed) -> Option<Result<Event, axum::Error>> {
    match change {
        Changed::Run { id, author } => runs::one(pool, id, &author).await.ok()?.map(run_event),
        Changed::Case {
            id,
            position,
            author,
        } => cases::one(pool, id, position, &author)
            .await
            .ok()?
            .map(case_event),
    }
}

fn run_event(run: Run) -> Result<Event, axum::Error> {
    Event::default().event("run").json_data(run)
}

fn case_event(case: CaseResult) -> Result<Event, axum::Error> {
    Event::default().event("case").json_data(case)
}

fn received(receiver: broadcast::Receiver<Changed>) -> impl Stream<Item = Changed> {
    stream::unfold(receiver, |mut receiver| async move {
        let change = receiver.recv().await.ok()?;
        Some((change, receiver))
    })
}
