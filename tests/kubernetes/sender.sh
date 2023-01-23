set -e
SENDER_EXECUTABLE="https://storage.googleapis.com/zombienet-db-snaps/stps/sender"

if [ ! -s sender ]; then
    curl -o sender $SENDER_EXECUTABLE
    chmod +x sender
fi

./sender --node-url ws://127.0.0.1:9944 --sender-index $1 --total-senders $2 -n $3
