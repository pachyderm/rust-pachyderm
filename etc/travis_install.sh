#!/bin/bash

set -ex

mkdir -p $HOME/cached-deps

if [ ! -d "$HOME/cached-deps/pachyderm" ]; then
    wget -c https://github.com/pachyderm/pachyderm/archive/v${PACHYDERM_VERSION}.zip
    unzip v${PACHYDERM_VERSION}
    mv pachyderm-${PACHYDERM_VERSION} $HOME/cached-deps/pachyderm
fi

pushd $HOME/cached-deps/pachyderm
    sudo etc/testing/travis_cache.sh
    sudo etc/testing/travis_install.sh
popd

if [ ! -f "$HOME/cached-deps/pachctl.deb" ]; then
    curl -o $HOME/cached-deps/pachctl.deb -L https://github.com/pachyderm/pachyderm/releases/download/v${PACHYDERM_VERSION}/pachctl_${PACHYDERM_VERSION}_amd64.deb
fi
sudo dpkg -i $HOME/cached-deps/pachctl.deb
