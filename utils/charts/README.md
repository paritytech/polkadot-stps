# helm chart registries

To run a chart-registry locally, execute the `chartmuseum.sh` script in this directory.
This will create a directory `/chartmuseum-storage`, and run the `chartmuseum` docker image locally.

## Pushing images to local registry

To do this, first you need a helm chart directory (such sa `/sender/` and `/tps/`). Then do the following:

```
$ helm package .
```

This will create a `.tgz` file. 

Then, to upload this to the locally running `chartmuseum` server, do the following:

```
$ curl --data-binary "@mychart-0.1.0.tgz" http://localhost:8080/api/charts
```

## Removing charts
Simply delete the chart from the `chartmuseum-storage/` directory.