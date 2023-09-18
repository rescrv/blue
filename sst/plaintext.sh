N=100000000

BINDIR=../target/release

# Generate keys and values separately using armnod
$BINDIR/armnod --number $N --chooser-mode set-once --cardinality $N --length-mode uniform --string-min-length 8 --string-max-length 32 > keys
$BINDIR/armnod --number $N --chooser-mode random --length-mode uniform --string-min-length 64 --string-max-length 256 > values

# Combine the keys and values into "{} {}\n" format.
paste keys values > key-value-pairs
rm keys values

# Split the keys and values into 128MB tables.
split --line-bytes 128M -x --additional-suffix .txt key-value-pairs table
rm key-value-pairs

# Convert each table to an sst.
for table in table*.txt
do
    LC_ALL=C sort -S 256M -o $table $table
    rm -f ${table:r}.sst
    $BINDIR/sst-from-plaintext --plaintext ${table} --output ${table:r}.sst
done
