# Waku v2

Waku is the result of work from [VAC](https://vac.dev/), the R&D division of [Status](https://status.im). It is the evolution of [Whisper](https://eips.ethereum.org/EIPS/eip-627), the original Ethereum messaging protocol over ÐΞVp2p.

This implementation does not concern Waku v1, and goes directly towards Waku v2.

Waku v2 implements a PubSub service over [libp2p](https://libp2p.io/), in addition to:
- retrieving historical messages for mostly off-line devices
- adaptive nodes
- bandwidth preserving

As a first iteration, `waku-rs` aims to cover the following specs, which are listed [as the recommendations for a minimal implementation](https://rfc.vac.dev/spec/10/#recommendations-for-clients) of a new Waku v2 client:
- [10/WAKU2](https://rfc.vac.dev/spec/10) - main spec
- [11/WAKU2-RELAY](https://rfc.vac.dev/spec/11) - for basic operation
- [14/WAKU2-MESSAGE](https://rfc.vac.dev/spec/14) - version 0 (unencrypted)
- [13/WAKU2-STORE](https://rfc.vac.dev/spec/13) - for historical messaging (query mode only)
- [19/WAKU2-LIGHTPUSH](https://rfc.vac.dev/spec/19) - for pushing messages

## Protocol IDs

Currently, `waku-rs` cares about the current `libp2p` protocol identifiers proposed by Waku:
- `/vac/waku/relay/2.0.0`
- `/vac/waku/store/2.0.0-beta4`
- `/vac/waku/lightpush/2.0.0-beta1`

Messages are exchanged over a [bi-directional binary stream](https://docs.libp2p.io/concepts/protocols/). Therefore, `libp2p` protocols prefix binary message payloads with the length of the message in bytes. The length integer is encoded as a [protobuf varint](https://developers.google.com/protocol-buffers/docs/encoding#varints).

## Networking Domains

Waku proposes three domains for networking activity:
- gossip
- discovery
- request/response

### Gossip

Waku gossips to disseminate messages throughout the network.
The gossip protocol identifier under `libp2p` is `/vac/waku/relay/2.0.0`, and is specified under [11/WAKU2-RELAY](https://rfc.vac.dev/spec/11).

The [23/WAKU2-TOPICS](https://rfc.vac.dev/spec/23) provides specifications for the recommended topic usage.

### Discovery

`waku-rs` uses DNS-based discovery to retrieve a list of nodes to connect to, defined by [EIP-1459](https://eips.ethereum.org/EIPS/eip-1459).

### Request/Response

Waku provides the following Request/Response protocols, which are designed for low bandwidth and being mostly offline.
- [12/WAKU2-FILTER](https://rfc.vac.dev/spec/12) - content filtering: makes fetching of a subset of messages bandwidth preserving - `/vac/waku/filter/2.0.0-beta1`
- [19/WAKU2-LIGHTPUSH](https://rfc.vac.dev/spec/19) - light push: used for nodes with short connection windows and limited bandwidth to publish messages - `/vac/waku/lightpush/2.0.0-beta1`

## Transports

As a specification, Waku is transport agonistic.

`waku-rs` supports the TCP transport, both dialing and listening.

`waku-rs` supports secure websockets for bidirectional communication streams.

# Protocol Interaction

The diagram below displays an overview of how different protocol interact under Waku:

![waku diagram](https://rfc.vac.dev/rfcs/10/overview.png)

Say we have 6 nodes: **A-F**. The `pubtopic` and `pubtopic2` PubSub Topics indicate which topic each node is interested in under the relay routing scheme.

1. Node A creates `msg1`  with `contentTopic1` as Content Topic.
2. Node F requests to get messages filtered by `pubtopic1` and `contentTopic1`.
3. Node D starts forwarding messages that match `pubtopic1` and `contentTopic1 to F.
4. Node A publishes `msg1` on `pubtopic1`. The message gets relayed further from B to D, but not C.
5. Node D saves `msg1` for possible later retrieval by other nodes.
6. Node D pushes `msg1` to F, since it has previously subscribed F to this filter.
7. At a later time, E comes online. Then, it requests messages matching `pubtopic1` and `contentTopic1` from Node D. Node D with messages meeting this criteria.
