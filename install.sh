#!/usr/bin/env bash

SCRIPT_DIR=$(dirname $( readlink -e $0 ) )
echo $SCRIPT_DIR
if [[ $1 == "" || $1 == "dump-analyzer" ]]; then
    cargo install --path "${SCRIPT_DIR}/dump-analyzer" || exit 1
fi
