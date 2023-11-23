#!/usr/bin/env zsh

set -ex

N=10000000

BINDIR=../target/release

# Generate keys and values separately using armnod
$BINDIR/armnod --number $N --chooser-mode set-once --cardinality $N --length-mode uniform --string-min-length 8 --string-max-length 32 --charset alnum > keys
$BINDIR/armnod --number $N --chooser-mode random --length-mode uniform --string-min-length 64 --string-max-length 256 > values

# Combine the keys and values into "{} {}\n" format.
paste keys values > key-value-pairs
rm keys values

# Split the keys and values into 128MB tables.
split --line-bytes 64M -x --additional-suffix .txt key-value-pairs table
rm key-value-pairs

# Convert each table to an sst.
for table in table*.txt
do
    LC_ALL=C sort -S 256M -o $table $table
    rm -f ${table:r}.{log,log.sst,sst}
    # Must convert logs after sorting as plaintext timestamp is line number.
    $BINDIR/log-from-plaintext --plaintext ${table} --output ${table:r}.log
    $BINDIR/sst-from-plaintext --plaintext ${table} --output ${table:r}.sst
    $BINDIR/sst-from-log --input ${table:r}.log --output ${table:r}.log.sst
    set +x
    $BINDIR/log-checksum ${table:r}.log
    $BINDIR/sst-checksum ${table:r}.sst
    $BINDIR/sst-checksum ${table:r}.log.sst
    set -x
done

sha256sum table* >! CHECKSUMS
