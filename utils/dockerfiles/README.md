# Docker images

These are built and published to Parity's registries on releases via a GitHub Action.

It is also possible to build the images locally from the root directory in the repo as follows:

```
$ docker build -f utils/dockerfiles/Dockerfile.<binary> --build-arg CHAIN=$FEATURE --build-arg VCS_REF=default --build-arg BUILD_DATE=default -t stps-<binary>:$FEATURE-latest .
```