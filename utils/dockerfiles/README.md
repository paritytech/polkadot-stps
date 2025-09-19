# Docker images

These are built and published to Parity's registries on releases via a GitHub Action.

It is also possible to build the images locally from the root directory in the repo as follows:

## Sender
```sh
docker buildx build --platform=linux/amd64 -f utils/dockerfiles/Dockerfile.sender -t stps-sender:local --build-arg VCS_REF="$(git rev-parse --short HEAD)" --build-arg BUILD_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)" --load .
```

## Funder
```sh
docker buildx build --platform=linux/amd64 -f utils/dockerfiles/Dockerfile.funder -t stps-funder:local --build-arg VCS_REF="$(git rev-parse --short HEAD)" --build-arg BUILD_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)" --load .
```