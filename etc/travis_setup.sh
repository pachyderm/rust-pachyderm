#!/bin/bash

set -ex

docker version
which pachctl

pushd $HOME/cached-deps/pachyderm
    make launch-kube
    pachctl deploy local
    until timeout 1s ./proto/pachyderm/etc/kube/check_ready.sh app=pachd; do
        sleep 1;
    done
    pachctl version
popd
