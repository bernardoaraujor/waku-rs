use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::NetworkBehaviour;
use waku_protocol::{
    waku_relay::network_behaviour::{WakuRelayBehaviour, WakuRelayEvent},
    waku_store::network_behaviour::{WakuStoreBehaviour, WakuStoreEvent},
};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WakuNodeEvent")]
pub struct WakuNodeBehaviour {
    relay: Toggle<WakuRelayBehaviour>,
    store: Toggle<WakuStoreBehaviour>,
}

#[derive(Debug)]
pub enum WakuNodeEvent {
    WakuRelayBehaviour(WakuRelayEvent),
    WakuStoreBehaviour(WakuStoreEvent),
}

impl From<WakuRelayEvent> for WakuNodeEvent {
    fn from(event: WakuRelayEvent) -> Self {
        Self::WakuRelayBehaviour(event)
    }
}

impl From<WakuStoreEvent> for WakuNodeEvent {
    fn from(event: WakuStoreEvent) -> Self {
        Self::WakuStoreBehaviour(event)
    }
}

#[derive(Debug)]
pub enum WakuNodeError {
    RelayNotEnabled,
    RelaySubscriptionErr,
}

impl WakuNodeBehaviour {
    pub fn new(relay_enabled: bool, store_enabled: bool, store_capacity: usize) -> Self {
        let relay = match relay_enabled {
            true => Toggle::from(Some(WakuRelayBehaviour::new())),
            false => Toggle::from(None),
        };

        let store = match store_enabled {
            true => Toggle::from(Some(WakuStoreBehaviour::new(store_capacity))),
            false => Toggle::from(None),
        };

        WakuNodeBehaviour { relay, store }
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<(), WakuNodeError> {
        match self.relay.as_mut() {
            Some(r) => match r.subscribe(topic) {
                Ok(_) => return Ok(()),
                Err(_) => return Err(WakuNodeError::RelaySubscriptionErr),
            },
            None => return Err(WakuNodeError::RelayNotEnabled),
        }
    }
}
