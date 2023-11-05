#!/usr/bin/env zsh

N=1000000000

BINDIR=../target/release

# Generate keys and values separately using armnod
$BINDIR/armnod --number $N --chooser-mode set-once --cardinality $N --length-mode uniform --string-min-length 8 --string-max-length 16 --charset alnum > keys
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
    $BINDIR/sst-from-plaintext --plaintext ${table} --output ${table:r}.sst --timestamp
    $BINDIR/sst-checksum ${table:r}.sst
done
