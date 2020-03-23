c-lightning-pruning-plugin
==========================

This plugin manages pruning of bitcoind such that it can always sync

## Command line options

- `pruning-interval`
    - number of seconds to wait between pruning checks
    - default: `600`

## Installation and Usage

Install `cargo`
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### From Crates.io

```
cargo install c-lightning-pruning-plugin
lightningd --plugin=~/.cargo/bin/c-lightning-pruning-plugin
```

### From Source

```
git clone https://github.com/Start9Labs/c-lightning-pruning-plugin.git
cd c-lightning-pruning-plugin
cargo build --release
lightningd --plugin=/path/to/c-lightning-pruning-plugin/target/release/c-lightning-pruning-plugin
```
