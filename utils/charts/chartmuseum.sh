#!/bin/bash

docker run --rm -it \
  -p 8080:8080 \
  -e DEBUG=1 \
  -e STORAGE=local \
  -e STORAGE_LOCAL_ROOTDIR=/chartmuseum-storage \
  -v $(pwd)/chartmuseum-storage:/charts \
  ghcr.io/helm/chartmuseum:v0.15.0