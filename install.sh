#!/usr/bin/env bash

SCRIPT_DIR=$(dirname $(readlink -e $0))
echo $SCRIPT_DIR
if [[ $1 == "" || $1 == "rust-memory-analyzer" ]]; then
    cargo install --path "${SCRIPT_DIR}/rust-memory-analyzer" || exit 1
fi
if [[ $1 == "" || $1 == "example-target" ]]; then
    cargo install --path "${SCRIPT_DIR}/example-target" --debug || exit 1
fi
