SHELL := /bin/bash

PACHYDERM_ROOT?=${GOPATH}/src/github.com/pachyderm/pachyderm
RUST_PACHYDERM_ROOT?=${PWD}

.PHONY: clean init

clean:
	rm -rf proto

proto:
	mkdir proto
	cd $(PACHYDERM_ROOT)/src && find ./client -maxdepth 5 -regex ".*\.proto" -exec cp --parents {} $(RUST_PACHYDERM_ROOT)/proto/ \;
	find ./proto -name "*.proto" -exec sed -i '' 's/import.*gogo.proto.*\;//' {} +
	find ./proto -name "*.proto" -exec sed -i '' 's/\[.*gogoproto.*\]//' {} +

init:
	git submodule update --init
