#!/bin/bash

set -e

POLKADOT_V=v0.9.33
ZOMBIENET_V=v1.3.5
CLUSTER_ID="gke_parity-zombienet_europe-west3-b_parity-zombienet"

print_help() {
  echo "Polkadot sTPS"
  echo ""
  echo "$ ./polkadot-stps.sh init"
  echo "$ ./polkadot-stps.sh test tests/relay.feature"
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
    wget https://github.com/paritytech/polkadot/releases/download/$POLKADOT_V/polkadot
    chmod +x polkadot
  fi
}

install_polkadotjs() {
  if [[ $(node -e "require('@polkadot/api')" &> /dev/null; echo $?) -gt 0 ]]; then
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
    tar -xf google-cloud-cli-382.0.0-linux-x86_64.tar.gz
    ./google-cloud-sdk/install.sh
  fi
}

init_gcloud() {
  # if ! xxx; then
    # gcloud auth login
  # fi
  if [[ $(kubectl config current-context) != $CLUSTER_ID ]]; then
    echo "setting up kubectl context for gcloud cluster..." 
    gcloud container clusters get-credentials parity-zombienet --zone europe-west3-b --project parity-zombienet
  fi
}

stps_test() {
  stps_init
  export PATH=.:$PATH
  ./zombienet-linux test --provider kubernetes $1
}

stps_init() {
  install_polkadotjs
  install_kubectl
  install_gcloud
  init_gcloud
  fetch_zombienet
  fetch_polkadot
}

subcommand=$1
case $subcommand in
  "" | "-h" | "--help")
    print_help
    ;;
  *)
    shift
    stps_${subcommand} $@
    if [ $? = 127 ]; then
      echo "Error: '$subcommand' is not a known subcommand." >&2
      echo "Run './polkadot-stps.sh --help' for a list of known subcommands." >&2
        exit 1
    fi
  ;;
esac
