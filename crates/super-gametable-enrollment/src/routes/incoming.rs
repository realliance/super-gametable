use axum::{
    extract::{Query, State},
    response::sse::{Event, Sse},
};
use futures::stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::{Stream, StreamExt};

use crate::EnrollmentServer;

#[derive(Deserialize, Debug)]
struct EnrollmentQuery {
    pub api_key: Option<String>,
}

async fn incoming_enrollment_handler(
    Query(enrollment_query): Query<EnrollmentQuery>,
    State(mut enrollment_state): State<EnrollmentServer>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    todo!();

    let stream = stream::repeat_with(|| Event::default().data("hi!"))
        .map(Ok)
        .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
