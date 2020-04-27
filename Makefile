SHELL := /bin/bash

PACHYDERM_ROOT?=${GOPATH}/src/github.com/pachyderm/pachyderm
RUST_PACHYDERM_ROOT?=${PWD}
PACHD_ADDRESS?="grpc://$(shell minikube ip):30650"

.PHONY: clean init fuzz-extract-restore

clean:
	rm -rf proto

proto:
	mkdir proto
	cd $(PACHYDERM_ROOT)/src && find ./client -maxdepth 5 -regex ".*\.proto" -exec cp --parents {} $(RUST_PACHYDERM_ROOT)/proto/ \;
	./etc/fix_protos.sh
	cargo build

fuzz-extract-restore:
	pachctl create repo fuzz_extract_restore_input
	pachctl create pipeline -f fuzz/etc/extract_restore_output.json
	RUST_BACKTRACE=1 PACHD_ADDRESS=$(PACHD_ADDRESS) cargo fuzz run extract_restore
	yes | pachctl delete all
