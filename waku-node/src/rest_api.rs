use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str, sync::Arc};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};
use waku_protocol::waku_message::WakuMessage;
use warp::{http::StatusCode, path::Tail, reply, Filter, Rejection, Reply};

type Result<T> = std::result::Result<T, Rejection>;
type RelayCache = Arc<Mutex<HashMap<String, Vec<WakuMessageSerDe>>>>;

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone, Debug)]
struct WakuMessageSerDe {
    payload: String,
    contentTopic: String,
    version: u32,
    timestamp: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PubSubTopicsSerDe {
    topics: Vec<String>,
}

pub async fn serve(
    mut relay_cache_rx: Receiver<(WakuMessage, String)>,
    relay_publish_tx: Sender<(WakuMessage, String)>,
    relay_subscribe_tx: Sender<Vec<String>>,
    relay_unsubscribe_tx: Sender<Vec<String>>,
) {
    let relay_cache: RelayCache = Arc::new(Mutex::new(HashMap::new()));
    let relay_cache_ref = relay_cache.clone();

    let get_relay_v1_messages_topic_route = warp::get()
        .and(warp::path("relay"))
        .and(warp::path("v1"))
        .and(warp::path("messages"))
        .and(warp::path::tail().map(|tail: Tail| tail.as_str().to_string()))
        .and(warp::any().map(move || relay_cache_ref.clone()))
        .and_then(get_relay_v1_messages_topic);

    let post_relay_v1_messages_topic_route = warp::post()
        .and(warp::path("relay"))
        .and(warp::path("v1"))
        .and(warp::path("messages"))
        .and(warp::path::tail().map(|tail: Tail| tail.as_str().to_string()))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and(warp::any().map(move || relay_publish_tx.clone()))
        .and_then(post_relay_v1_messages_topic);

    let post_relay_v1_subscriptions = warp::post()
        .and(warp::path("relay"))
        .and(warp::path("v1"))
        .and(warp::path("subscriptions"))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and(warp::any().map(move || relay_subscribe_tx.clone()))
        .and_then(post_relay_v1_subscriptions);

    let delete_relay_v1_subscriptions = warp::delete()
        .and(warp::path("relay"))
        .and(warp::path("v1"))
        .and(warp::path("subscriptions"))
        .and(warp::body::content_length_limit(1024 * 16).and(warp::body::json()))
        .and(warp::any().map(move || relay_unsubscribe_tx.clone()))
        .and_then(delete_relay_v1_subscriptions);

    let routes = get_relay_v1_messages_topic_route
        .or(post_relay_v1_messages_topic_route)
        .or(post_relay_v1_subscriptions)
        .or(delete_relay_v1_subscriptions);
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
                contentTopic: waku_message.get_content_topic().to_string(),
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

async fn post_relay_v1_messages_topic(
    topic: String,
    msg: WakuMessageSerDe,
    relay_post_tx: Sender<(WakuMessage, String)>,
) -> Result<impl Reply> {
    let mut waku_message = WakuMessage::new();
    waku_message.set_payload(msg.payload.as_bytes().to_vec());
    waku_message.set_content_topic(msg.contentTopic);
    waku_message.set_timestamp(msg.timestamp);
    waku_message.set_version(msg.version);

    match relay_post_tx.send((waku_message, topic)).await {
        Ok(_) => Ok(reply::with_status("", StatusCode::OK)),
        Err(_) => Ok(reply::with_status("", StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

async fn post_relay_v1_subscriptions(
    topics: PubSubTopicsSerDe,
    relay_subscribe_tx: Sender<Vec<String>>,
) -> Result<impl Reply> {
    match relay_subscribe_tx.send(topics.topics).await {
        Ok(_) => Ok(reply::with_status("", StatusCode::OK)),
        Err(_) => Ok(reply::with_status("", StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

async fn delete_relay_v1_subscriptions(
    topics: PubSubTopicsSerDe,
    relay_subscribe_tx: Sender<Vec<String>>,
) -> Result<impl Reply> {
    match relay_subscribe_tx.send(topics.topics).await {
        Ok(_) => Ok(reply::with_status("", StatusCode::OK)),
        Err(_) => Ok(reply::with_status("", StatusCode::INTERNAL_SERVER_ERROR)),
    }
}
