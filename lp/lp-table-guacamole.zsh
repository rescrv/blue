#!/usr/bin/env zsh

set -e

../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8
rm lp-table-guacamole.sst

../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 1 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 10 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 1 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 0 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 8 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 64 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 1 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 1 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 2 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 4 --prev-probability 0.5
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.01
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.1
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.25
rm lp-table-guacamole.sst
../target/debug/lp-table-guacamole --num-keys 100 --key-bytes 8 --value-bytes 128 --num-seeks 10 --seek-distance 8 --prev-probability 0.5
rm lp-table-guacamole.sst

echo '======='
echo 'SUCCESS'
echo '======='
