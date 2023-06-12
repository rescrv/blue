#!/usr/bin/env zsh
for script in scripts/*
do
    ../target/debug/texttale $script \
        | sha256sum \
        | sed -e "s, -\$,$script,"
done >! CHECKSUMS
