use crate::network_behaviour::WakuNodeEvent;
use clap::Parser;
use libp2p::{
    futures::StreamExt, gossipsub::GossipsubEvent, identity::Keypair, swarm::Swarm,
    swarm::SwarmEvent, Multiaddr, PeerId,
};
use log::info;
use network_behaviour::WakuNodeBehaviour;
use protobuf::Message;
use std::error::Error;
use tokio::sync::mpsc;
use waku_protocol::{
    waku_lightpush::network_behaviour::WakuLightPushEvent,
    waku_message::WakuMessage,
    waku_relay::network_behaviour::{WakuRelayEvent, DEFAULT_PUBSUB_TOPIC},
    waku_store::network_behaviour::WakuStoreEvent,
};

mod network_behaviour;
mod rest_api;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Enable relay protocol
    #[clap(long, action = clap::ArgAction::Set, default_value = "true")]
    relay: bool,

    /// Multiaddr of peer to directly connect with. Option may be repeated
    #[clap(long)]
    static_node: Option<Vec<Multiaddr>>,

    /// List of topics to listen
    #[clap(long)]
    topics: Option<Vec<String>>,

    /// Enable store protocol
    #[clap(long, action = clap::ArgAction::Set, default_value = "false")]
    store: bool,

    /// Maximum number of messages to store
    #[clap(long, default_value = "50000")]
    store_capacity: usize,

    /// Enable lightpush protocol
    #[clap(long, action = clap::ArgAction::Set, default_value = "false")]
    lightpush: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Cli::parse();

    let local_key = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local peer id: {:?}", local_peer_id);

    let transport = libp2p::development_transport(local_key.clone()).await?;

    if args.store && args.lightpush {
        panic!(
            "This implementation cannot run Store and LightPush at the same time! \
            Please check https://github.com/bernardoaraujor/waku-rs/issues/1 "
        );
    }

    let mut waku_node_behaviour =
        WakuNodeBehaviour::new(args.relay, args.store, args.store_capacity, args.lightpush);

    match args.topics {
        Some(topics) => {
            for t in topics {
                waku_node_behaviour.subscribe(&t).unwrap();
            }
        }
        None => waku_node_behaviour.subscribe(DEFAULT_PUBSUB_TOPIC).unwrap(),
    }

    let mut swarm = Swarm::new(transport, waku_node_behaviour, local_peer_id);
    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    match args.static_node {
        Some(addresses) => {
            for a in addresses {
                match swarm.dial(a.clone()) {
                    Ok(_) => info!("Dialed {:?}", a),
                    Err(e) => panic!("Failed to dial address: {:?} {:?}", a, e),
                }
            }
        }
        None => {}
    }

    let (relay_cache_tx, relay_cache_rx) = mpsc::channel(32);
    tokio::spawn(rest_api::serve(relay_cache_rx));

    loop {
        let event = swarm.select_next_some().await;
        info!("{:?}", event);
        match event {
            SwarmEvent::Behaviour(WakuNodeEvent::WakuRelayBehaviour(
                WakuRelayEvent::GossipSub(GossipsubEvent::Message {
                    propagation_source: _,
                    message_id: _,
                    message,
                }),
            ))
            | SwarmEvent::Behaviour(WakuNodeEvent::WakuStoreBehaviour(
                WakuStoreEvent::WakuRelayBehaviour(WakuRelayEvent::GossipSub(
                    GossipsubEvent::Message {
                        propagation_source: _,
                        message_id: _,
                        message,
                    },
                )),
            ))
            | SwarmEvent::Behaviour(WakuNodeEvent::WakuLightPushBehaviour(
                WakuLightPushEvent::WakuRelayBehaviour(WakuRelayEvent::GossipSub(
                    GossipsubEvent::Message {
                        propagation_source: _,
                        message_id: _,
                        message,
                    },
                )),
            )) => {
                let mut waku_message = WakuMessage::new();
                waku_message.merge_from_bytes(&message.data).unwrap();
                relay_cache_tx.send(waku_message).await.unwrap();
            }
            _ => {}
        }
    }

    // Ok(())
}
