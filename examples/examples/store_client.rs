//! First, start a Waku node with Store enabled:
//!
//! ```sh
//! cargo run -- --store
//! ```
//!
//! You should take note of the MultiAddr (/ip4/127.0.0.1/tcp/xxxxx)
//! and the PeerId (xxxxxxxxxxxxxxxxxxxxxx_peer_id_xxxxxxxxxxxxxxxxxxxxx).
//! Then on a new terminal, run the example:
//!
//! ```sh
//! cargo run --example store_client -- /waku/2/default-waku/proto /ip4/127.0.0.1/tcp/xxxxx xxxxxxxxxxxxxxxxxxxxxx_peer_id_xxxxxxxxxxxxxxxxxxxxx
//! ```
//!
//! Every line you feed into stdin will be the unencrypted payload of a message in a query
//! where the content topic is defined by the CONTENT_TOPIC constant.
//!
//! If no relay or lightpush has fed any messages to the Store node, all queries will be unsuccessful.
//! You should run the relay or lightpush examples before this one.

use async_std::io;
use async_std::io::prelude::BufReadExt;
use futures::select;
use libp2p::futures::StreamExt;
use libp2p::{identity::Keypair, swarm::Swarm, Multiaddr, PeerId};
use log::info;
use std::error::Error;
use waku_protocol::waku_message::WakuMessage;
use waku_protocol::waku_store::network_behaviour::{compute_index, WakuStoreBehaviour};

const CONTENT_TOPIC: &str = "content_topic";

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let local_key = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local peer id: {:?}", local_peer_id);

    let transport = libp2p::development_transport(local_key.clone()).await?;

    let mut waku_store_behaviour = WakuStoreBehaviour::new(10);

    let pubsub_topic = match std::env::args().nth(1) {
        Some(t) => t,
        None => panic!("No pubsub topic provided!"),
    };

    waku_store_behaviour.subscribe(&pubsub_topic).unwrap();

    let mut swarm = Swarm::new(transport, waku_store_behaviour, local_peer_id);
    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    match std::env::args().nth(2) {
        Some(to_dial) => {
            let address: Multiaddr = to_dial.parse().expect("User to provide valid address.");
            match swarm.dial(address.clone()) {
                Ok(_) => info!("Dialed {:?}", address),
                Err(e) => panic!("failed to dial address: {:?} {:?}", address, e),
            }
        }
        None => panic!("No MultiAddress provided!"),
    }

    let peer_id = match std::env::args().nth(3) {
        Some(s) => match s.parse() {
            Ok(p) => p,
            Err(_) => panic!("Cannot parse provided PeerId!"),
        },
        None => panic!("No PeerId provided!"),
    };

    let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();

    loop {
        select! {
            line = stdin.select_next_some() => {
                let mut msg = WakuMessage::new();
                msg.set_payload(line.expect("Stdin not to close").as_bytes().to_vec());
                msg.set_content_topic(CONTENT_TOPIC.to_string());

                let cursor = compute_index(msg);

                let mut content_topics = Vec::new();
                content_topics.push(CONTENT_TOPIC.to_string());
                swarm.behaviour_mut().send_query(
                    peer_id,
                    "test_request_id".to_string(),
                    cursor,
                    1,
                    true,
                    pubsub_topic.to_string(),
                    content_topics,
                );
            },
            event = swarm.select_next_some() => {
                info!("{:?}", event);
            }
        }
    }
}
