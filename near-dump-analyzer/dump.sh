#!/usr/bin/env bash

if [[ $1 == "" ]]; then
    echo NEAR pid = $(pidof neard)
    echo "usage ./dump.sh <PID> [TID]"
    exit 1
fi
PID=$1
TID=$2

make bins/dump
mkdir -p /tmp/dump/logs;
mkdir -p /tmp/dump/logs || true;
mkdir -p /tmp/dump/symbols || true;
pushd $(dirname $(readlink /proc/${PID}/exe))
test -f /tmp/dump/symbols/${PID} || (echo maint print psymbols | sudo gdb -p "${PID}" >> "/tmp/dump/symbols/${PID}");
test -f /tmp/dump/symbols/${PID}.m || (echo maint print msymbols | sudo gdb -p "${PID}" >> "/tmp/dump/symbols/${PID}.m");
popd
sudo ${PWD}/bins/dump "${PID}" ${TID}
