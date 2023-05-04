#!/bin/bash

docker run --rm -it -d \
  -p 8080:8080 \
  -e DEBUG=1 \
  -e STORAGE=local \
  -e STORAGE_LOCAL_ROOTDIR=/charts \
  --name helm-repository \
  -v $(pwd)/charts:/charts \
  ghcr.io/helm/chartmuseum:v0.15.0

  sudo chmod 777 charts

  cd sender && helm package . && curl --data-binary "@stps-sender-0.1.0.tgz" http://localhost:8080/api/charts && cd ..
  cd tps && helm package . && curl --data-binary "@stps-tps-scraper-0.1.0.tgz" http://localhost:8080/api/charts && cd ..
