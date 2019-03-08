proto/extracted:
	mkdir proto/extracted
	cd ./proto/pachyderm/src/client && find . -maxdepth 4 -regex ".*\.proto" -exec cp --parents {} ../../../extracted/ \;

init:
	git submodule update --init
	make proto/extracted

.PHONY: init
