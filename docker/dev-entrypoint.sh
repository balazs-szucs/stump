#!/usr/bin/env bash

set -euo pipefail

cd /workspace

# On bind mounts, node_modules may start empty; install once and reuse the volume.
if [ ! -d node_modules ] || [ ! -f node_modules/.yarn-integrity ]; then
  yarn install --frozen-lockfile
fi

exec "$@"
