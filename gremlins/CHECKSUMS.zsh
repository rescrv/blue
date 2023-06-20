#!/usr/bin/env zsh
set -e
for script in scripts/*
do
    ../target/debug/gremlins \
        --control-center-listener-host localhost \
        --control-center-listener-port 1982 \
        --control-center-listener-ca-file ca.pem \
        --control-center-listener-private-key-file home.key \
        --control-center-listener-certificate-file home.crt \
        $script \
        | sha256sum \
        | sed -e "s, -\$,$script,"
done >! CHECKSUMS
