set -e

# Usage: ./sender.sh <port> <total_senders> <num_of_transactions> <relaychain_or_parachain>

PORT=$1
TOTAL_SENDERS=$2
NUM_TRX=$3
RELAY_OR_PARA=$4

if [ $# -eq 0 ]
  then
    echo "No arguments supplied! \nUsage: ./sender.sh <port> <total_senders> <num_of_transactions> <relaychain_or_parachain> <chain_metadata>"
    exit
fi

if [ $RELAY_OR_PARA == "relaychain" ]
  then
    SENDER_EXECUTABLE="https://github.com/paritytech/polkadot-stps/releases/download/v0.1.0-alpha/sender-rococo-linux-x86"
  else
    echo "Must set the sender executable to use the rococo metadata when testing Relaychain TPS!"
    exit
fi

if [ $RELAY_OR_PARA == "parachain" ]
  then
    SENDER_EXECUTABLE="https://github.com/paritytech/polkadot-stps/releases/download/v0.1.0-alpha/sender-polkadot-parachain-linux-x86"
  else
    echo "Must set the sender executable to use the polkadot-parachain metadata when testing Parachain TPS!"
fi

if [ ! -s sender ]
  then
    curl -o /tmp/sender $SENDER_EXECUTABLE
    chmod +x /tmp/sender
fi

SENDER_INDEX=`cat /tmp/sender_index`

/tmp/sender --node-url ws://127.0.0.1:$PORT --total-senders $TOTAL_SENDERS --num $NUM_TRX --sender-index $SENDER_INDEX