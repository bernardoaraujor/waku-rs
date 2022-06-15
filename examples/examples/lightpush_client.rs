//! First, connect a Waku node with LightPush enabled to another running Waku node:
//!
//! ```sh
//! cargo run -- --lightpush --static-node /ip4/127.0.0.1/tcp/yyyyy
//! ```
//!
//! You should take note of the MultiAddr (/ip4/127.0.0.1/tcp/xxxxx)
//! and the PeerId (xxxxxxxxxxxxxxxxxxxxxx_peer_id_xxxxxxxxxxxxxxxxxxxxx).
//! Then on a new terminal, run the example:
//!
//! ```sh
//! cargo run --example lightpush_client /waku/2/default-waku/proto /ip4/127.0.0.1/tcp/xxxxx xxxxxxxxxxxxxxxxxxxxxx_peer_id_xxxxxxxxxxxxxxxxxxxxx
//! ```
//! Every line you feed into stdin will be the unencrypted payload of a message to be pushed,
//! where the content topic is defined by the CONTENT_TOPIC constant.

use async_std::io;
use async_std::io::prelude::BufReadExt;
use futures::select;
use libp2p::futures::StreamExt;
use libp2p::{identity::Keypair, swarm::Swarm, PeerId};
use log::info;
use std::error::Error;
use waku_protocol::waku_lightpush::network_behaviour::WakuLightPushBehaviour;
use waku_protocol::waku_message::WakuMessage;

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

    let waku_lp_behaviour = WakuLightPushBehaviour::new();

    let pubsub_topic = match std::env::args().nth(1) {
        Some(t) => t,
        None => panic!("No pubsub topic provided!"),
    };

    let mut swarm = Swarm::new(transport, waku_lp_behaviour, local_peer_id);
    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    let peer_addr = match std::env::args().nth(2) {
        Some(s) => match s.parse() {
            Ok(p) => p,
            Err(_) => panic!("Cannot parse provided MultiAddr!"),
        },
        None => panic!("No MultiAddr provided!"),
    };

    let peer_id = match std::env::args().nth(3) {
        Some(s) => match s.parse() {
            Ok(p) => p,
            Err(_) => panic!("Cannot parse provided PeerId!"),
        },
        None => panic!("No PeerId provided!"),
    };

    swarm.behaviour_mut().add_lightpush_peer(peer_id, peer_addr);

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();

    let mut req_id = 0;
    loop {
        select! {
            line = stdin.select_next_some() => {
                let mut msg = WakuMessage::new();
                msg.set_payload(line.expect("Stdin not to close").as_bytes().to_vec());
                msg.set_content_topic(CONTENT_TOPIC.to_string());

                swarm.behaviour_mut().send_request(peer_id, req_id.to_string(), pubsub_topic.clone(), msg);
                req_id += 1;
            },
            event = swarm.select_next_some() => {
                info!("{:?}", event);
            }
        }
    }
}
