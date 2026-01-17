#!/bin/sh
set -e

# Start nginx in background
nginx -g "daemon off;" &

# Start cdc-cli in foreground
exec cdc-cli start