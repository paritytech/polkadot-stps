#!/bin/bash

# Note that this script assumes a registry running in minikube for local development
# which is port-forwarded locally on port 5000, or alternatively, a registry running in a container
# with the same port exposede.

docker tag stps-tps localhost:5000/stps-tps:latest
docker push localhost:5000/stps-tps:latest

docker tag stps-sender localhost:5000/stps-sender:latest
docker push localhost:5000/stps-sender:latest

docker tag stps-funder localhost:5000/stps-funder:latest
docker push localhost:5000/stps-funder:latest