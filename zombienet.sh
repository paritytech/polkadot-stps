#!/bin/sh

POLKADOT_V=v0.9.17-rc4
ZOMBIENET_V=v1.2.14

fetch_zombienet() {
  if [ ! -s bin/zombienet-linux ]; then
    echo "fetching zombienet executable..."
    wget --quiet --directory-prefix bin https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/zombienet-linux
    chmod +x bin/zombienet-linux
  fi
}

fetch_polkadot() {
  if [ ! -s bin/polkadot ]; then
    echo "fetching polkadot executable..."
    wget --quiet --directory-prefix bin https://github.com/paritytech/polkadot/releases/download/$POLKADOT_V/polkadot
    chmod +x bin/polkadot
  fi
}

build_collator() {
  if [ ! -L bin/parachain-collator ]; then
    echo "building collator executable..."
    cargo build --release --quiet
    ln -s target/release/parachain-collator bin/parachain-collator
  fi
}

run_ecosystem_benchmarks() {
  if [ ! -d bin ]; then
    mkdir bin
  fi

  fetch_zombienet
  fetch_polkadot
  build_collator

  ./bin/zombienet-linux -p native test tests/examples/0001-simple-network.feature
}

run_ecosystem_benchmarks
