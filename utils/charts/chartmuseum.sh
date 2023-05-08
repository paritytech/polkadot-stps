#!/bin/bash

docker run --rm -it -d \
  -p 8080:8080 \
  -e DEBUG=1 \
  -e STORAGE=local \
  -e STORAGE_LOCAL_ROOTDIR=/charts \
  --name chartmuseum \
  -v $(pwd)/charts:/charts \
  ghcr.io/helm/chartmuseum:v0.15.0

sudo chmod 777 charts

helm repo add chartmuseum http://localhost:8080

cd sender && helm package . && curl --data-binary "@stps-sender-0.1.0.tgz" http://localhost:8080/api/charts && cd ..
cd tps && helm package . && curl --data-binary "@stps-tps-scraper-0.1.0.tgz" http://localhost:8080/api/charts && cd ..
