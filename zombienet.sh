#!/bin/bash

POLKADOT_V=v0.9.17-rc4
ZOMBIENET_V=v1.2.27

print_help() {
  echo "🧟 Zombienet - Polkadot Ecosystem Performance Benchmarks 🦾"
  echo ""
  echo "we are about to spin a polkadot relay chain with a parachain node with runtime extrinsics to be tested against."
  echo "first, create a pallet for your team, including the extrinsics you want to run tests for."
  echo "make sure you read zombienet specs from it's official repo: https://github.com/paritytech/zombienet"
  echo "write the zombienet test specifications under the tests directory"
  echo "then, call this script:"
  echo "$ ./zombienet.sh init"
  echo "$ ./zombienet.sh test tests/examples/0001-simple-network.feature"
  echo "$ ./zombienet.sh spawn tests/examples/0001-simple-network.toml"
}

fetch_zombienet() {
  if [ ! -s zombienet-linux ]; then
    echo "fetching zombienet executable..."
    wget --quiet https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/zombienet-linux
    chmod +x zombienet-linux
  fi
}

fetch_polkadot() {
  if [ ! -s polkadot ]; then
    echo "fetching polkadot executable..."
    wget --quiet https://github.com/paritytech/polkadot/releases/download/$POLKADOT_V/polkadot
    chmod +x polkadot
  fi
}

install_polkadotjs() {
  if [[ ! $(npm list | grep polkadot) ]]; then
    echo "installing polkadot-js..."
    npm install @polkadot/api
  fi
}

install_kubectl() {
  if ! command -v kubectl &> /dev/null; then
    echo "installing kubectl..."
    curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
    sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl
  fi
}

install_gcloud() {
  if ! command -v gcloud &> /dev/null; then
    echo "installing gcloud"
    curl -O https://dl.google.com/dl/cloudsdk/channels/rapid/downloads/google-cloud-cli-382.0.0-linux-x86_64.tar.gz
    tar -xf google-cloud-cli-382.0.0-linux-x86.tar.gz
    ./google-cloud-sdk/install.sh
  fi
}

# todo: init_gcloud

build_collator() {
  if [ ! -s target/release/parachain-collator ]; then
    echo "building collator executable..."
    cargo build --release
  fi
}

zombienet_test() {
  zombienet_init
  ./zombienet-linux test --provider native $1
}

zombienet_spawn() {
  zombienet_init
  ./zombienet-linux spawn --provider native $1
}

zombienet_init() {
  install_polkadotjs
  install_kubectl
  install_gcloud
  # todo: init_gcloud
  fetch_zombienet
  fetch_polkadot
  build_collator
}

subcommand=$1
case $subcommand in
  "" | "-h" | "--help")
    print_help
    ;;
  *)
    shift
    zombienet_${subcommand} $@
    if [ $? = 127 ]; then
      echo "Error: '$subcommand' is not a known subcommand." >&2
      echo "Run './zombienet.sh --help' for a list of known subcommands." >&2
        exit 1
    fi
  ;;
esac
