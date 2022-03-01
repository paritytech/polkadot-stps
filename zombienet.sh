#!/bin/sh

POLKADOT_V=v0.9.17-rc4
ZOMBIENET_V=v1.2.14

print_help() {
  echo "🧟 Zombienet Ecosystem Performance Optimizations 🦾"
  echo ""
  echo "we are about to spin a polkadot relay chain with a parachain node with extrinsics to be tested against."
  echo "first, create a pallet for your team, including the extrinsics you want to run tests for."
  echo "make sure you read zombienet specs from it's official repo: https://github.com/paritytech/zombienet"
  echo "write the zombienet test specifications under the tests directory"
  echo "then, call this script with the following parameters:"
  echo "$ ./zombienet.sh test <team_name> <test_name>"
  echo ""
}

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

while getopts ":h" option; do
   case $option in
      h) # display Help
         print_help
         exit;;
   esac
done

run_ecosystem_benchmarks
