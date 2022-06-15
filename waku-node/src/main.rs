use clap::Parser;
use libp2p::futures::StreamExt;
use libp2p::{identity::Keypair, swarm::Swarm, Multiaddr, PeerId};
use log::info;
use network_behaviour::WakuNodeBehaviour;
use std::error::Error;
use waku_protocol::waku_relay::network_behaviour::DEFAULT_PUBSUB_TOPIC;

pub mod network_behaviour;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Enable relay protocol [default: true]
    #[clap(long)]
    relay: Option<bool>,

    /// Multiaddr of peer to directly connect with. Option may be repeated
    #[clap(long)]
    static_node: Option<Vec<Multiaddr>>,

    /// List of topics to listen
    #[clap(long)]
    topics: Option<Vec<String>>,

    /// Enable store protocol [default: false]
    #[clap(long)]
    store: Option<bool>,

    /// Maximum number of messages to store
    #[clap(long, default_value = "50000")]
    store_capacity: usize,

    /// Enable lightpush protocol [default: false]
    #[clap(long)]
    lightpush: Option<bool>,
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Cli::parse();

    let local_key = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local peer id: {:?}", local_peer_id);

    let transport = libp2p::development_transport(local_key.clone()).await?;

    let relay_enabled = match args.relay {
        Some(r) => r,
        None => true,
    };

    let store_enabled = match args.store {
        Some(s) => s,
        None => false,
    };

    let lightpush_enabled = match args.lightpush {
        Some(l) => l,
        None => false,
    };

    let mut waku_node_behaviour = WakuNodeBehaviour::new(
        relay_enabled,
        store_enabled,
        args.store_capacity,
        lightpush_enabled,
    );

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

    loop {
        let event = swarm.select_next_some().await;
        info!("{:?}", event);
    }

    // Ok(())
}
