#!/bin/bash

mkdir -p $HOME/cached-deps

if [ ! -d "$HOME/cached-deps/pachyderm" ]; then
    wget -c https://github.com/pachyderm/pachyderm/archive/v$(PACHYDERM_VERSION).zip
    unzip v$(PACHYDERM_VERSION)
    mv pachyderm-$(PACHYDERM_VERSION) pachyderm
fi

pushd pachyderm
    sudo etc/testing/travis_cache.sh && \
    sudo etc/testing/travis_install.sh && \
    curl -o /tmp/pachctl.deb -L https://github.com/pachyderm/pachyderm/releases/download/v$(PACHYDERM_VERSION)/pachctl_$(PACHYDERM_VERSION)_amd64.deb  && \
    sudo dpkg -i /tmp/pachctl.deb
popd
