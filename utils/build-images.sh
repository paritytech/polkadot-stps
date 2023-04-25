#!/bin/bash

# Build tps image
docker build -f dockerfiles/Dockerfile.tps -t stps-tps:latest .

# Build sender image
docker build -f dockerfiles/Dockerfile.sender -t stps-sender:latest .

# Build funder image
docker build -f dockerfiles/Dockerfile.funder -t stps-funder:latest .
