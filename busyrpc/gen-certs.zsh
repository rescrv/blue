#!/usr/bin/env zsh

set -ex

mkdir -p certs

cat > certs/ssl.conf << EOF
[ ca ]
default_ca  = CA_default

[ CA_default ]

dir = ./ca
certs = $dir/certs
crl_dir = $dir/crl
database = $dir/index.txt
new_certs_dir = $dir/newcerts

certificate = $dir/cacert.pem
serial = $dir/serial
crlnumber = $dir/crlnumber
crl = $dir/crl.pem
private_key = $dir/private/cakey.pem

x509_extensions = usr_cert

name_opt = ca_default
cert_opt = ca_default

default_days = 365
default_crl_days= 30
default_md = default
preserve = no

policy = policy_anything

[ policy_anything ]
countryName = optional
stateOrProvinceName = optional
localityName = optional
organizationName = optional
organizationalUnitName = optional
commonName = supplied
emailAddress = optional

[ req ]
default_bits = 2048
default_keyfile = privkey.pem
distinguished_name = req_distinguished_name
attributes = req_attributes
x509_extensions = v3_ca # The extensions to add to the self signed cert

input_password = secret
output_password = secret

string_mask = utf8only

req_extensions = v3_req

[ req_distinguished_name ]
countryName = US
stateOrProvinceName = CA
localityName = SanFrancisco
organizationName = None
organizationalUnitName = None
commonName = localhost
emailAddress = root@localhost

[ req_attributes ]
challengePassword = A challenge password
challengePassword_min = 4
challengePassword_max = 20

unstructuredName = An optional company name

[ usr_cert ]

basicConstraints=CA:FALSE
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid,issuer

[ v3_req ]
subjectAltName=DNS:localhost,DNS:localhost.localdomain,IP:127.0.0.1,IP:::1

[ v3_ca ]
EOF

if test '!' -f certs/ca.key
then
    openssl req \
        -new -x509 \
        -subj "/C=US/ST=CA/L=SanFrancisco/O=private/OU=private/CN=localhost" \
        -passout pass:password \
        -keyout certs/ca.key \
        -out certs/ca.crt \
        -config certs/ssl.conf
fi

gen_server() {
    SERVER="$1"
    shift
    openssl genrsa -out "certs/$SERVER.key"
    openssl req \
        -new -nodes \
        -subj "/C=US/ST=CA/L=SanFrancisco/O=private/OU=private/CN=localhost" \
        -sha256 \
        -extensions v3_req \
        -key "certs/$SERVER.key" \
        -out "certs/$SERVER.csr" \
        -config certs/ssl.conf
    openssl x509 \
        -req \
        -passin pass:password \
        -in "certs/$SERVER.csr" \
        -CA certs/ca.crt \
        -CAkey certs/ca.key \
        -CAcreateserial \
        -extfile certs/ssl.conf \
        -extensions v3_req \
        -out "certs/$SERVER.crt"
}

for x in $@
do
    gen_server $x
done
