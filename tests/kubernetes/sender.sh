set -e

if [ $4 == "relay"]
    then
        SENDER_EXECUTABLE="https://storage.googleapis.com/zombienet-db-snaps/stps/sender"
fi

# Currently WIP, don't expect full functionality for this yet.
if [ $4 == "para" ]
    then
        SENDER_EXECUTABLE="https://github.com/bredamatt/releases/releases/download/stps-test/sender-linux-x86"
fi

if [ ! -s sender ]; then
    /cfg/curl -o /tmp/sender $SENDER_EXECUTABLE
    chmod +x /tmp/sender
fi

SENDER_INDEX=`cat /tmp/sender_index`

/tmp/sender --node-url ws://127.0.0.1:$1 --total-senders $2 -n $3 --sender-index $SENDER_INDEX