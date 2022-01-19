#!/usr/bin/env bash

set -ve

SCRIPT_DIR=$(dirname $(readlink -e $0))

# Install all binaries
${SCRIPT_DIR}/install.sh

# kill previous
pkill example-target || echo "nothing to kill"

# Start example, which uses 8gb of ram
example-target &
sleep 10

# Measure memory usage
sudo $(which rust-memory-analyzer) analyze --pid `pidof example-target`

