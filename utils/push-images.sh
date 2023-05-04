#!/bin/bash

# Note that this script assumes a registry running in minikube for local development
# which is port-forwarded locally on port 5000, or alternatively, a registry running in a container
# with the same port exposede.

if [ -z "$1" ]
    then
        echo 'No argument supplied. Must pass either "rococo", or "tick".'
        exit 1
fi

FEATURE=$1

docker tag stps-tps-$FEATURE localhost:5000/stps-tps-$FEATURE:latest
docker push localhost:5000/stps-tps-$FEATURE:latest

docker tag stps-sender-$FEATURE localhost:5000/stps-sender-$FEATURE:latest
docker push localhost:5000/stps-sender-$FEATURE:latest

docker tag stps-funder-$FEATURE localhost:5000/stps-funder-$FEATURE:latest
docker push localhost:5000/stps-funder-$FEATURE:latest
