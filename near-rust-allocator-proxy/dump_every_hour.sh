#!/usr/bin/env bash


mkdir -p hourly_dumps

for i in {000000..1000000}; do
	echo DUMP $i
	make dump | tee hourly_dumps/dump003.$i
	sleep 300
done
