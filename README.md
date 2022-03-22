# waku-rs

[Waku](https://wakunetwork.com/)  is a p2p messaging protocol tailored for the web3, with origins in Ethereum's [Whisper](https://vac.dev/fixing-whisper-with-waku).

This Rust implementation is taking reference from the [official specifications of Waku v2](https://rfc.vac.dev/spec/10/), as well as the [Nim](https://github.com/status-im/nim-waku), [JavaScript](https://github.com/status-im/js-waku) and [Go](https://github.com/status-im/go-waku) implementations.

Check [docs](docs/README.md) for more informataion.

# ⚠️ warning⚠️

This is a very early experimental project, don't create any expectations based on it. It has no official affiliation with [VAC](https://vac.dev/) nor [Status](https://status.im/).

# requirements

In order to compile `waku-rs`, you need `protoc` on your path. On Ubuntu:
```sh
$ sudo apt install -y protobuf-compiler
```
