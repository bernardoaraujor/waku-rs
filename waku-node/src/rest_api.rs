use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str, sync::Arc};
use tokio::sync::{mpsc::Receiver, Mutex};
use waku_protocol::waku_message::WakuMessage;
use warp::{http::StatusCode, path::Tail, reply, Filter, Rejection, Reply};

type Result<T> = std::result::Result<T, Rejection>;
type RelayCache = Arc<Mutex<HashMap<String, Vec<WakuMessageSerDe>>>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct WakuMessageSerDe {
    payload: String,
    content_topic: String,
    version: u32,
    timestamp: i64,
}

pub async fn serve(mut relay_cache_rx: Receiver<(WakuMessage, String)>) {
    let relay_cache: RelayCache = Arc::new(Mutex::new(HashMap::new()));
    let relay_cache_ref = relay_cache.clone();

    let get_relay_v1_messages_topic_route = warp::get()
        .and(warp::path("relay"))
        .and(warp::path("v1"))
        .and(warp::path("messages"))
        .and(warp::path::tail().map(|tail: Tail| tail.as_str().to_string()))
        .and(warp::any().map(move || relay_cache_ref.clone()))
        .and_then(get_relay_v1_messages_topic);

    let routes = get_relay_v1_messages_topic_route;
    tokio::spawn(warp::serve(routes).run(([127, 0, 0, 1], 5000)));

    while let Some((waku_message, topic)) = relay_cache_rx.recv().await {
        let mut unlock_relay_cache = relay_cache.lock().await;
        unlock_relay_cache
            .entry(topic)
            .or_insert_with(Vec::new)
            .push(WakuMessageSerDe {
                payload: str::from_utf8(waku_message.get_payload())
                    .unwrap()
                    .to_string(),
                content_topic: waku_message.get_content_topic().to_string(),
                version: waku_message.get_version(),
                timestamp: waku_message.get_timestamp(),
            });
    }
}

async fn get_relay_v1_messages_topic(topic: String, relay_cache: RelayCache) -> Result<impl Reply> {
    let mut local_relay_cache = relay_cache.lock().await;
    let messages = match local_relay_cache.contains_key(&topic) {
        true => local_relay_cache[&topic].clone(),
        false => vec![],
    };
    local_relay_cache.remove(&topic);
    Ok(reply::with_status(reply::json(&messages), StatusCode::OK))
}
