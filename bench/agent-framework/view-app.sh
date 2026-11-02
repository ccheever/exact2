#!/bin/bash
# Serve one agent's built app for viewing: NAME IMAGE APPDIR APPPORT APIPORT.
# The API and the app listen on the same port numbers inside and out, so the
# browser and the app's server reach the API at one URL (HOSTNAME_SEEN: the address
# a browser on another machine uses; default localhost).
set -e
NAME=$1 IMAGE=$2 APPDIR=$3 APPPORT=$4 APIPORT=$5 HOSTNAME_SEEN=${HOSTNAME_SEEN:-localhost}
export DOCKER_CONTEXT=colima-bench
docker rm -f "view-$NAME" >/dev/null 2>&1 || true
docker run -d --name "view-$NAME" -p "$APPPORT:$APPPORT" -p "$APIPORT:$APIPORT" -v "$APPDIR:/src:ro" \
  --entrypoint bash "$IMAGE" -c "
    set -e
    cd /home/agent/work && rm -rf app && cp -a /src app
    PORT=$APIPORT PUBLIC_URL=http://$HOSTNAME_SEEN:$APIPORT LATENCY_MS=0 API_OVERLAY= node api/server.mjs > /tmp/api.log 2>&1 &
    cd app
    export API_BASE_URL=http://$HOSTNAME_SEEN:$APIPORT
    npm install --no-audit --no-fund --include=dev > /tmp/install.log 2>&1
    NODE_ENV=production npm run build > /tmp/build.log 2>&1
    echo BUILT
    exec env PORT=$APPPORT HOST=0.0.0.0 HOSTNAME=0.0.0.0 NODE_ENV=production npm start
  " >/dev/null
echo "view-$NAME started"
