#!/bin/sh

POLKADOT_V=v0.9.17-rc4
ZOMBIENET_V=v1.2.14

# because zombienet is packaged into an executable, we need some workarounds (steam-run) to execute it under NixOS
if grep -q 'NAME=NixOS' /etc/os-release; then
  EXECUTABLE_PREFIX="steam-run"
else
  EXECUTABLE_PREFIX=""
fi

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

  $EXECUTABLE_PREFIX ./bin/zombienet-linux
}

run_ecosystem_benchmarks
