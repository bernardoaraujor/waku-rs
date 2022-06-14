// this file is being used for development purposes!
// cargo run --bin light_push_client -- x_pubsub_topic_x /ip4/127.0.0.1/tcp/xxxxx xxxxxxxxxxxxxxxxxxxxxx_peer_id_xxxxxxxxxxxxxxxxxxxxx

use async_std::io;
use async_std::io::prelude::BufReadExt;
use futures::select;
use libp2p::futures::StreamExt;
use libp2p::{identity::Keypair, swarm::Swarm, PeerId};
use log::info;
use std::error::Error;
use waku_protocol::waku_lightpush::network_behaviour::WakuLightPushBehaviour;
use waku_protocol::waku_message::WakuMessage;

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

    let peer_addr = std::env::args().nth(2).unwrap().parse().unwrap();

    let peer_id = std::env::args().nth(3).unwrap().parse().unwrap();

    swarm.behaviour_mut().add_lightpush_peer(peer_id, peer_addr);

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();

    loop {
        select! {
            line = stdin.select_next_some() => {
                let mut msg = WakuMessage::new();
                let content_topic = "test_content_topic";
                msg.set_payload(line.expect("Stdin not to close").as_bytes().to_vec());
                msg.set_content_topic(content_topic.to_string());

                swarm.behaviour_mut().send_request(peer_id, "test_request_id".to_string(), pubsub_topic.clone(), msg);
            },
            event = swarm.select_next_some() => {
                info!("{:?}", event);
            }
        }
    }
}
