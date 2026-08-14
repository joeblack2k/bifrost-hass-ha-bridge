use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use futures::StreamExt;
use futures::stream::{self, Stream};
use hue::event::EventBlock;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use crate::error::{ApiError, ApiResult};
use crate::server::appstate::AppState;
use crate::server::hueevents::{HueEventRecord, HueEventReplay};

fn event_from_block(block: EventBlock, id: Option<String>) -> Result<Event, axum::Error> {
    let json = [block];
    log::trace!(
        "## EVENT ##: {}",
        serde_json::to_string(&json).unwrap_or_else(|_| "ERROR".to_string())
    );
    let event = id.map_or_else(Event::default, |id| Event::default().id(id));
    event.json_data(json)
}

fn event_from_record(record: HueEventRecord) -> Result<Event, axum::Error> {
    let id = record.id();
    event_from_block(record.block, Some(id))
}

#[allow(clippy::result_large_err)]
fn api_event_from_record(record: HueEventRecord) -> ApiResult<Event> {
    event_from_record(record).map_err(ApiError::from)
}

pub async fn get_clip_v2(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = ApiResult<Event>>> {
    let hello = tokio_stream::iter([Ok(Event::default().comment("hi"))]);
    let last_event_id = headers.get("last-event-id").map(HeaderValue::to_str);

    let (channel, replay) = {
        let lock = state.res.lock().await;
        let channel = lock.hue_event_stream().subscribe();
        let replay = match last_event_id {
            Some(Ok(id)) => match lock.hue_event_stream().events_sent_after_id(id) {
                HueEventReplay::After(events) => {
                    events.into_iter().map(api_event_from_record).collect()
                }
                HueEventReplay::SnapshotRequired { checkpoint } => {
                    vec![
                        event_from_block(EventBlock::add(lock.get_resources()), checkpoint)
                            .map_err(ApiError::from),
                    ]
                }
            },
            _ => Vec::new(),
        };
        (channel, replay)
    };

    let live = BroadcastStream::new(channel).scan(false, |terminated, event| {
        if *terminated {
            return futures::future::ready(None);
        }

        futures::future::ready(Some(match event {
            Ok(record) => api_event_from_record(record),
            Err(err @ BroadcastStreamRecvError::Lagged(missed)) => {
                *terminated = true;
                log::warn!("Hue SSE stream lagged; missed {missed} events");
                Err(err.into())
            }
        }))
    });
    let events = stream::iter(replay).chain(live).boxed();

    // Hue clients (especially on mobile) rely on a long-lived SSE connection to get realtime
    // updates; without keep-alives, intermediaries/OSes can silently tear down the stream.
    Sse::new(hello.chain(events)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text(": ping"),
    )
}

pub fn router() -> Router<AppState> {
    Router::new().route("/clip/v2", get(get_clip_v2))
}
