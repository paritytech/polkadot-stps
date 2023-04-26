set -e
SENDER_EXECUTABLE="https://storage.googleapis.com/zombienet-db-snaps/stps/sender"

if [ ! -s sender ]; then
    curl -o /tmp/sender $SENDER_EXECUTABLE
    chmod +x /tmp/sender
fi

SENDER_INDEX=`cat /tmp/sender_index`

/tmp/sender --node-url ws://127.0.0.1:$1 --total-senders $2 -n $3 --sender-index $SENDER_INDEX