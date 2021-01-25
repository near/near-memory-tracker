ifeq ($(REPO_PATH),)
	REPO_PATH := "../../nearcore.measure-mem"
endif


ifeq ($(NETWORK),)
	NETWORK := "testnet"
endif

PID := $(shell pidof neard)

all: bins/dump bins/near-c-allocator-proxy.so

bins/near-c-allocator-proxy.so: near-c-allocator-proxy.c
	mkdir -p bins
	gcc -O2 -shared -fPIC -o bins/near-c-allocator-proxy.so near-c-allocator-proxy.c

bins/dump: dump.cpp
	mkdir -p bins
	g++ -O2 -Wall -fsanitize=address -Warray-bounds -fbounds-check -g dump.cpp -o bins/dump --std=c++17

run: all
	rm -rf ${HOME}/.nearup/logs/*
	rm -rf /tmp/logs
	mkdir -p /tmp/logs
	# rm -f ${REPO_PATH}/target/debug/near
	cat $(REPO_PATH)/git_log_merge_base | head -3
	cd ${REPO_PATH}; (test -f Cargo.toml && (time cargo build -p neard --bin neard --features $(FEATURES) --release || rm target/debug/neard && exit 1) || echo no Cargo.toml)
	nearup stop
	rm -rf logs
	mkdir -p logs
	LD_PRELOAD=./bins/near-c-allocator-proxy.so nohup nice -10 nearup run $(NETWORK) --binary-path $(REPO_PATH)/target/release $(EXTRA)


dump: all
	echo PID=$(PID) ${PID};
	# rm -rf symbols;
	mkdir -p /tmp/logs;
	mkdir -p symbols || true;
	test -f symbols/${PID} || (echo maint print psymbols | sudo gdb -p "${PID}" >> "symbols/${PID}");
	test -f symbols/${PID}.m || (echo maint print msymbols | sudo gdb -p "${PID}" >> "symbols/${PID}.m");
	sudo ./bins/dump "${PID}"
