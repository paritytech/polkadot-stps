#!/bin/bash

if [ -z "$1" ]
    then 
        echo 'No argument supplied. Must pass either "rococo", or "tick".'
        exit 1
fi

FEATURE=$1

# Build tps image
docker build -f utils/dockerfiles/Dockerfile.tps --build-arg feature_flag=$FEATURE -t stps-tps-$FEATURE:latest .

# Build sender image
docker build -f utils/dockerfiles/Dockerfile.sender --build-arg feature_flag=$FEATURE -t stps-sender-$FEATURE:latest .

# Build funder image
docker build -f utils/dockerfiles/Dockerfile.funder --build-arg feature_flag=$FEATURE -t stps-funder-$FEATURE:latest .
