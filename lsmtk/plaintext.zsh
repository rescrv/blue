#!/usr/bin/env zsh

set -e
set -x

N=100000000

# Generate keys and values separately using armnod
cargo run --release -p armnod --bin armnod -- --chooser-mode set-once --cardinality $N --length-mode uniform --min-length 8 --max-length 16 --charset alnum > keys
cargo run --release -p armnod --bin armnod -- --chooser-mode random --length-mode uniform --min-length 512 --max-length 1536 | head -$N> values

# Combine the keys and values into "{} {}\n" format.
paste keys values > key-value-pairs
rm keys values

# Split the keys and values into 16MB tables.  This corresponds to 16k ops/s.
split --line-bytes 16M -x --additional-suffix .txt key-value-pairs table
rm key-value-pairs

# Convert each table to an sst.
for table in table*.txt
do
    LC_ALL=C sort -S 256M -o $table $table
    rm -f ${table:r}.sst
    cargo run --release -p sst --bin sst-from-plaintext -- --plaintext ${table} --output ${table:r}.sst --timestamp
    rm ${table}
done
