use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::NetworkBehaviour;
use waku_protocol::{
    waku_lightpush::network_behaviour::{WakuLightPushBehaviour, WakuLightPushEvent},
    waku_relay::network_behaviour::{WakuRelayBehaviour, WakuRelayEvent},
    waku_store::network_behaviour::{WakuStoreBehaviour, WakuStoreEvent},
};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WakuNodeEvent")]
pub struct WakuNodeBehaviour {
    relay: Toggle<WakuRelayBehaviour>,
    store: Toggle<WakuStoreBehaviour>,
    lightpush: Toggle<WakuLightPushBehaviour>,
}

#[derive(Debug)]
pub enum WakuNodeEvent {
    WakuRelayBehaviour(WakuRelayEvent),
    WakuStoreBehaviour(WakuStoreEvent),
    WakuLightPushBehaviour(WakuLightPushEvent),
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

impl From<WakuLightPushEvent> for WakuNodeEvent {
    fn from(event: WakuLightPushEvent) -> Self {
        Self::WakuLightPushBehaviour(event)
    }
}

#[derive(Debug)]
pub enum WakuNodeError {
    RelayNotEnabled,
    RelaySubscriptionErr,
}

impl WakuNodeBehaviour {
    pub fn new(
        relay_enabled: bool,
        store_enabled: bool,
        store_capacity: usize,
        lightpush_enabled: bool,
    ) -> Self {
        let relay = match relay_enabled {
            true => Toggle::from(Some(WakuRelayBehaviour::new())),
            false => Toggle::from(None),
        };

        let store = match store_enabled {
            true => Toggle::from(Some(WakuStoreBehaviour::new(store_capacity))),
            false => Toggle::from(None),
        };

        let lightpush = match lightpush_enabled {
            true => Toggle::from(Some(WakuLightPushBehaviour::new())),
            false => Toggle::from(None),
        };

        WakuNodeBehaviour {
            relay,
            store,
            lightpush,
        }
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
