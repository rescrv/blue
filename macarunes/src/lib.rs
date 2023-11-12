#![doc = include_str!("../README.md")]

use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::fmt::{Debug, Write};

use biometrics::Counter;

use buffertk::stack_pack;

use prototk_derive::Message;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const SIGNATURE_BYTES: usize = 32;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static VERIFIER_FIRST_PARTY_FAILURE: Counter = Counter::new("macaroons.verifier.1st_party_failure");
static VERIFIER_FIRST_PARTY_SUCCESS: Counter = Counter::new("macaroons.verifier.1st_party_success");
static VERIFIER_THIRD_PARTY_FAILURE: Counter = Counter::new("macaroons.verifier.3rd_party_failure");
static VERIFIER_THIRD_PARTY_SUCCESS: Counter = Counter::new("macaroons.verifier.3rd_party_success");

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// Error cases.
#[derive(Clone, Debug, Default, Message)]
pub enum Error {
    #[prototk(294912, message)]
    #[default]
    Success,
    #[prototk(294913, message)]
    Cycle,
    #[prototk(294914, message)]
    ProofInvalid,
    #[prototk(294915, message)]
    MissingLoader {
        #[prototk(1, string)]
        what: String,
    }
}

////////////////////////////////////////////// Caveat //////////////////////////////////////////////

#[derive(Clone, Message, Eq, PartialEq)]
enum Caveat {
    #[prototk(1, message)]
    ExactString {
        #[prototk(1, string)]
        what: String,
    },
    #[prototk(2, message)]
    Expires {
        #[prototk(1, uint64)]
        when: u64,
    },
    #[prototk(15, message)]
    ThirdParty {
        #[prototk(1, string)]
        location: String,
        #[prototk(2, string)]
        identifier: String,
        #[prototk(3, message)]
        secret: ThirdPartySecret,
    },
}

impl Caveat {
    pub fn is_first_party(&self) -> bool {
        match self {
            Caveat::ExactString { .. } => true,
            Caveat::Expires { .. } => true,
            Caveat::ThirdParty { .. } => false,
        }
    }

    pub fn is_third_party(&self) -> bool {
        match self {
            Caveat::ExactString { .. } => false,
            Caveat::Expires { .. } => false,
            Caveat::ThirdParty { .. } => true,
        }
    }
}

impl Default for Caveat {
    fn default() -> Self {
        Self::Expires { when: 0 }
    }
}

impl Debug for Caveat {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Caveat::ExactString { what } => {
                write!(fmt, "exact-string: what={:#?}", what)?;
            }
            Caveat::Expires { when } => {
                write!(fmt, "expires: when={}", when)?;
            }
            Caveat::ThirdParty {
                location,
                identifier,
                ..
            } => {
                write!(
                    fmt,
                    "third-party: location={} identifier={}",
                    location, identifier
                )?;
            }
        }
        Ok(())
    }
}

////////////////////////////////////////////// crypto //////////////////////////////////////////////

mod crypto {
    use super::*;

    pub const NONCE_BYTES: usize =
        libsodium_sys::crypto_secretbox_xsalsa20poly1305_NONCEBYTES as usize;
    pub const ZERO_BYTES: usize =
        libsodium_sys::crypto_secretbox_xsalsa20poly1305_ZEROBYTES as usize;
    pub const BOX_ZERO_BYTES: usize =
        libsodium_sys::crypto_secretbox_xsalsa20poly1305_BOXZEROBYTES as usize;

    /// hmac is not public.  Everything should use a wrapper in this module.
    fn hmac(key: &Secret, message: &[u8], out: &mut Secret) {
        unsafe {
            libsodium_sys::crypto_auth_hmacsha256(
                out.bytes.as_mut_ptr(),
                message.as_ptr(),
                message.len() as u64,
                key.bytes.as_ptr(),
            );
        }
    }

    pub fn explicit_bzero(bytes: &mut [u8]) {
        unsafe {
            libsodium_sys::sodium_memzero(bytes.as_mut_ptr() as *mut c_void, bytes.len());
        }
    }

    pub fn random_bytes(bytes: &mut [u8]) {
        unsafe {
            libsodium_sys::randombytes_buf(bytes.as_mut_ptr() as *mut c_void, bytes.len());
        }
    }

    pub fn mem_eq(lhs: &[u8], rhs: &[u8]) -> bool {
        let compared = unsafe {
            libsodium_sys::sodium_memcmp(
                lhs.as_ptr() as *mut c_void,
                rhs.as_ptr() as *mut c_void,
                std::cmp::min(lhs.len(), rhs.len()),
            )
        };
        compared == 0 && lhs.len() == rhs.len()
    }

    pub fn str_eq(lhs: &str, rhs: &str) -> bool {
        mem_eq(lhs.as_bytes(), rhs.as_bytes())
    }

    pub fn chained_hmac_1(key: &mut Secret, message: &[u8]) {
        let orig = Secret { bytes: key.bytes };
        hmac(&orig, message, key);
    }

    pub fn chained_hmac_n(key: &mut Secret, messages: &[&[u8]]) {
        let mut intermediaries = Vec::new();
        for message in messages {
            let mut sig = key.clone();
            hmac(key, message, &mut sig);
            intermediaries.push(sig);
        }
        let orig = key.clone();
        for digest in intermediaries.into_iter() {
            hmac(&orig, &digest.bytes, key);
        }
    }

    pub fn bind_for_request(root: &Macaroon, sig: &mut Secret) {
        chained_hmac_1(sig, &root.signature.bytes)
    }

    pub fn secretbox_xsalsa20poly1305(
        key: &[u8; SIGNATURE_BYTES],
        nonce: &[u8; NONCE_BYTES],
        plaintext: &[u8; ZERO_BYTES + SIGNATURE_BYTES],
        ciphertext: &mut [u8; ZERO_BYTES + SIGNATURE_BYTES],
    ) {
        for p in &plaintext[0..ZERO_BYTES] {
            assert_eq!(*p, 0);
        }
        let ret = unsafe {
            libsodium_sys::crypto_secretbox_xsalsa20poly1305(
                ciphertext.as_mut_ptr(),
                plaintext.as_ptr(),
                (ZERO_BYTES + SIGNATURE_BYTES) as u64,
                nonce.as_ptr(),
                key.as_ptr(),
            )
        };
        assert!(ret == 0);
    }

    #[must_use]
    pub fn secretbox_xsalsa20poly1305_open(
        key: &[u8; SIGNATURE_BYTES],
        nonce: &[u8; NONCE_BYTES],
        ciphertext: &[u8; ZERO_BYTES + SIGNATURE_BYTES],
        plaintext: &mut [u8; ZERO_BYTES + SIGNATURE_BYTES],
    ) -> bool {
        for c in &ciphertext[0..BOX_ZERO_BYTES] {
            assert_eq!(*c, 0);
        }
        let ret = unsafe {
            libsodium_sys::crypto_secretbox_xsalsa20poly1305_open(
                plaintext.as_mut_ptr(),
                ciphertext.as_ptr(),
                (ZERO_BYTES + SIGNATURE_BYTES) as u64,
                nonce.as_ptr(),
                key.as_ptr(),
            )
        };
        ret == 0
    }
}

////////////////////////////////////////////// Secret //////////////////////////////////////////////

/// A [Secret] holds [SIGNATURE_BYTES] that will be scrubbed at the end of its lifetime.
#[derive(Clone, Default, Message)]
pub struct Secret {
    #[prototk(1, bytes32)]
    bytes: [u8; SIGNATURE_BYTES],
}

impl Secret {
    pub fn hexdigest(&self) -> String {
        let mut hexdigest = String::with_capacity(2 * SIGNATURE_BYTES);
        for item in &self.bytes {
            write!(&mut hexdigest, "{:02x}", *item).expect("unable to write to string");
        }
        hexdigest
    }

    pub fn scrub(&mut self) {
        crypto::explicit_bzero(&mut self.bytes);
    }

    pub fn random() -> Self {
        let mut x = Self {
            bytes: [0u8; SIGNATURE_BYTES],
        };
        crypto::random_bytes(&mut x.bytes);
        x
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.scrub();
    }
}

impl Eq for Secret {}

impl PartialEq for Secret {
    fn eq(&self, other: &Secret) -> bool {
        crypto::mem_eq(&self.bytes, &other.bytes)
    }
}

///////////////////////////////////////// ThirdPartySecret /////////////////////////////////////////

/// A third party secret.  It is intended that this be generated by the server that holds a secret
/// for discharge macaroons.
#[derive(Clone, Default, Message)]
pub struct ThirdPartySecret {
    // NOTE(rescrv):  We give 32 bytes for the nonce, but will only ever use nonce[0..NONCE_BYTES],
    // which is less than 32.  We use a bytes32 to save from having to reference other structures.
    #[prototk(1, bytes32)]
    nonce: [u8; 32],
    // This is the 16 bytes that differs between plaintext and ciphertext.
    #[prototk(2, bytes16)]
    box_bytes: [u8; 16],
    #[prototk(3, bytes32)]
    data: [u8; 32],
}

impl ThirdPartySecret {
    /// Create a new [ThirdPartySecret] with the signature provided by the client and the nonce and
    /// discharge_secret provided by the server.
    pub fn new(signature: &Secret, nonce: &Secret, discharge_secret: &Secret) -> Self {
        use crypto::{BOX_ZERO_BYTES, NONCE_BYTES, ZERO_BYTES};
        let mut tps = ThirdPartySecret::default();
        let mut nonce_bytes = [0u8; NONCE_BYTES];
        nonce_bytes.copy_from_slice(&nonce.bytes[0..NONCE_BYTES]);
        let mut plaintext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
        plaintext[ZERO_BYTES..].copy_from_slice(&discharge_secret.bytes);
        let mut ciphertext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
        crypto::secretbox_xsalsa20poly1305(
            &signature.bytes,
            &nonce_bytes,
            &plaintext,
            &mut ciphertext,
        );
        tps.nonce[0..NONCE_BYTES].copy_from_slice(&nonce_bytes);
        tps.nonce[NONCE_BYTES..].copy_from_slice(&[0u8; 8]);
        tps.box_bytes.copy_from_slice(
            &ciphertext[BOX_ZERO_BYTES..BOX_ZERO_BYTES + (ZERO_BYTES - BOX_ZERO_BYTES)],
        );
        tps.data.copy_from_slice(&ciphertext[ZERO_BYTES..]);
        crypto::explicit_bzero(&mut nonce_bytes);
        crypto::explicit_bzero(&mut plaintext);
        crypto::explicit_bzero(&mut ciphertext);
        tps
    }

    /// Scrub the memory locations of the nonce and data.
    pub fn scrub(&mut self) {
        crypto::explicit_bzero(&mut self.nonce);
        crypto::explicit_bzero(&mut self.data);
    }
}

impl Drop for ThirdPartySecret {
    fn drop(&mut self) {
        self.scrub();
    }
}

impl Eq for ThirdPartySecret {}

impl PartialEq for ThirdPartySecret {
    fn eq(&self, other: &ThirdPartySecret) -> bool {
        crypto::mem_eq(&self.nonce, &other.nonce)
            && crypto::mem_eq(&self.box_bytes, &other.box_bytes)
            && crypto::mem_eq(&self.data, &other.data)
    }
}

///////////////////////////////////////////// Macaroon /////////////////////////////////////////////

/// A [Macaroon] has a location, identifier, signature, and caveats.
#[derive(Clone, Default, Message, Eq, PartialEq)]
pub struct Macaroon {
    #[prototk(1, string)]
    location: String,
    #[prototk(2, string)]
    identifier: String,
    #[prototk(3, message)]
    signature: Secret,
    #[prototk(4, message)]
    caveats: Vec<Caveat>,
}

impl Macaroon {
    /// Mint a new macaroon for the location, identifier, and root secret.
    pub fn new(location: String, identifier: String, mut secret: Secret) -> Self {
        crypto::chained_hmac_1(&mut secret, identifier.as_bytes());
        Macaroon {
            location,
            identifier,
            signature: secret,
            caveats: Vec::new(),
        }
    }

    /// The location of the macaroon.  This will be provided to the loader.  Intentionally not
    /// protected by the signature; it should be a hint to where the macaroon came from.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// The identifier for the macaroon.  This is how the server will translate the macaroon to its
    /// root secret.
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// The signature of the macaroon.  Keep it secret or any derived macaroons can be leaked.
    pub fn signature(&self) -> &Secret {
        &self.signature
    }

    /// Add a caveat that must match exactly.
    pub fn add_exact_string(&mut self, what: String) {
        self.add_caveat(Caveat::ExactString { what });
    }

    /// Add a caveat that expires the macaroon after when.
    pub fn add_expires(&mut self, when: u64) {
        self.add_caveat(Caveat::Expires { when });
    }

    /// Add a third party caveat.  Provide `signature()` to ask the third party to generate the
    /// identifier and secret.
    pub fn add_third_party_caveat(
        &mut self,
        location: String,
        identifier: String,
        secret: ThirdPartySecret,
    ) {
        self.add_caveat(Caveat::ThirdParty {
            location,
            identifier,
            secret,
        });
    }

    /// Bind a macaroon to the request to make sure discharge macaroons cannot be used in other
    /// contexts.
    pub fn bind_discharge(&self, discharge: &mut Macaroon) {
        crypto::bind_for_request(self, &mut discharge.signature);
    }

    fn add_caveat(&mut self, caveat: Caveat) {
        match &caveat {
            Caveat::ExactString { .. } | Caveat::Expires { .. } => {
                let buf = stack_pack(&caveat).to_vec();
                self.caveats.push(caveat);
                crypto::chained_hmac_1(&mut self.signature, &buf);
            }
            Caveat::ThirdParty {
                location: _,
                identifier,
                secret,
            } => {
                let messages = &[
                    identifier.as_bytes(),
                    &secret.nonce,
                    &secret.box_bytes,
                    &secret.data,
                ];
                crypto::chained_hmac_n(&mut self.signature, messages);
                self.caveats.push(caveat);
            }
        }
    }
}

impl Debug for Macaroon {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        writeln!(fmt, "location: {}", self.location)?;
        writeln!(fmt, "identifier: {}", self.identifier)?;
        writeln!(fmt, "signature: {}", self.signature.hexdigest())?;
        if self.caveats.len() == 1 {
            writeln!(fmt, "1 caveat")?;
        } else {
            writeln!(fmt, "{} caveats", self.caveats.len())?;
        }
        for caveat in &self.caveats {
            writeln!(fmt, "{:?}", caveat)?;
        }
        Ok(())
    }
}

////////////////////////////////////////////// Loader //////////////////////////////////////////////

/// A [Loader] is a client-side way to get macaroons by identifier.  Used by [RequestBuilder].
pub trait Loader: Debug {
    fn location(&self) -> &'static str;
    fn lookup(&self, identifier: &str) -> Result<Macaroon, Error>;
}

////////////////////////////////////////// RequestBuilder //////////////////////////////////////////

/// [RequestBuilder] handles assembly of requests, picking the minimal set of discharge macaroons.
#[derive(Debug, Default)]
pub struct RequestBuilder {
    loaders: HashMap<&'static str, Box<dyn Loader>>,
}

impl RequestBuilder {
    /// Create a new request builder.
    pub fn new() -> Self {
        Self {
            loaders: HashMap::new(),
        }
    }

    /// Add a loader for macaroons.  Every type of third party should have its own loader that's
    /// identified by `location()`.
    pub fn add_loader<L: Loader + 'static>(&mut self, loader: L) {
        self.loaders.insert(loader.location(), Box::new(loader));
    }

    /// Prepare a request with the location and identifier provided.
    pub fn prepare_request(&self, location: &str, identifier: &str) -> Result<(Macaroon, Vec<Macaroon>), Error> {
        let root = self.get_loader_for(location)?.lookup(identifier)?;
        let mut discharges = Vec::new();
        let mut queue = VecDeque::new();
        self.enqueue_discharges(&root, &mut discharges, &mut queue)?;
        while let Some(macaroon) = discharges.pop() {
            self.enqueue_discharges(&macaroon, &mut discharges, &mut queue)?;
        }
        Ok((root, discharges))
    }

    fn get_loader_for(&self, location: &str) -> Result<&dyn Loader, Error> {
        if let Some(loader) = self.loaders.get(location) {
            Ok(loader.as_ref())
        } else {
            Err(Error::MissingLoader {
                what: location.to_owned(),
            })
        }
    }

    fn enqueue_discharges(&self, macaroon: &Macaroon, discharges: &mut Vec<Macaroon>, queue: &mut VecDeque<usize>) -> Result<(), Error> {
        for caveat in macaroon.caveats.iter() {
            if let Caveat::ThirdParty { location, identifier, secret: _ } = caveat {
                let discharge = self.get_loader_for(location)?.lookup(identifier)?;
                queue.push_back(discharges.len());
                discharges.push(discharge);
            }
        }
        Ok(())
    }
}

///////////////////////////////////////////// Verifier /////////////////////////////////////////////

/// [Verifier] implements the servers-side portion of macaroons.  Note that the caveats we've
/// chosen require very little logic from the server.
#[derive(Clone, Debug, Default)]
pub struct Verifier {
    now: u64,
    contexts: Vec<String>,
}

impl Verifier {
    /// Add `context` to the context of the request.  Concretely adding a string here means the
    /// client can add a contextual caveat that we will verify.
    pub fn add_context(&mut self, context: String) {
        self.contexts.push(context);
    }

    /// Set the time to use for the verifier.
    pub fn set_current_time(&mut self, now: u64) {
        self.now = now;
    }

    /// Verify a macaroon.  Resistant to timing attacks, but quadratic as a result.
    pub fn verify(
        &self,
        root: &Macaroon,
        key: &Secret,
        discharges: &[Macaroon],
    ) -> Result<(), Error> {
        let mut key = key.clone();
        self.verify_inner(root, &mut key, root, discharges, 0)
    }

    // NOTE(rescrv):  This function is quadratic in nature.  For each discharge we will consider
    // every other discharge.  This is done to avoid timing attacks, but comes at a cost of
    // efficiency.
    fn verify_inner(
        &self,
        m: &Macaroon,
        secret: &mut Secret,
        tm: &Macaroon,
        discharges: &[Macaroon],
        depth: usize,
    ) -> Result<(), Error> {
        // recursive macaroons.
        if depth > discharges.len() {
            return Err(Error::Cycle);
        }
        crypto::chained_hmac_1(secret, m.identifier.as_bytes());
        let mut fail = false;

        for caveat in &m.caveats {
            let old_fail = fail;
            let counter = if caveat.is_first_party() {
                fail |= !self.verify_1st(secret, caveat);
                if !old_fail && fail {
                    &VERIFIER_FIRST_PARTY_FAILURE
                } else {
                    &VERIFIER_FIRST_PARTY_SUCCESS
                }
            } else if caveat.is_third_party() {
                fail |= !self.verify_3rd(secret, caveat, tm, discharges, depth);
                if !old_fail && fail {
                    &VERIFIER_THIRD_PARTY_FAILURE
                } else {
                    &VERIFIER_THIRD_PARTY_SUCCESS
                }
            } else {
                panic!("logic error regarding first and third parties");
            };
            counter.click();
        }

        if m != tm {
            crypto::bind_for_request(tm, &mut *secret);
        }

        fail |= *secret != m.signature;

        if fail {
            Err(Error::ProofInvalid)
        } else {
            Ok(())
        }
    }

    fn verify_1st(&self, secret: &mut Secret, caveat: &Caveat) -> bool {
        assert!(caveat.is_first_party());
        let mut found = false;
        let buf = stack_pack(caveat).to_vec();
        crypto::chained_hmac_1(secret, &buf);
        match caveat {
            Caveat::ExactString { what } => {
                for context in &self.contexts {
                    found |= crypto::mem_eq(context.as_bytes(), what.as_bytes());
                }
            }
            Caveat::Expires { when } => {
                found |= *when > self.now;
            }
            Caveat::ThirdParty { .. } => {
                panic!("this should be covered by the above assert");
            }
        }
        found
    }

    fn verify_3rd(
        &self,
        key: &mut Secret,
        caveat: &Caveat,
        tm: &Macaroon,
        discharges: &[Macaroon],
        depth: usize,
    ) -> bool {
        use crypto::{BOX_ZERO_BYTES, NONCE_BYTES, ZERO_BYTES};
        let mut fail = false;
        let mut found = false;

        if let Caveat::ThirdParty {
            location: _,
            identifier,
            secret,
        } = caveat
        {
            for (d_idx, discharge) in discharges.iter().enumerate() {
                if crypto::str_eq(&discharge.identifier, identifier) {
                    let mut nonce = [0u8; NONCE_BYTES];
                    let mut plaintext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
                    let mut ciphertext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
                    nonce.copy_from_slice(&secret.nonce[0..NONCE_BYTES]);
                    ciphertext[BOX_ZERO_BYTES..ZERO_BYTES].copy_from_slice(&secret.box_bytes);
                    ciphertext[ZERO_BYTES..].copy_from_slice(&secret.data);
                    fail |= !crypto::secretbox_xsalsa20poly1305_open(
                        &key.bytes,
                        &nonce,
                        &ciphertext,
                        &mut plaintext,
                    );
                    // NOTE(rescrv):  We drop the error here.  The only return value for
                    // verify_inner is ProofInvalid.  We know that will often be the case.
                    let mut inner_key = Secret::default();
                    inner_key.bytes.copy_from_slice(&plaintext[ZERO_BYTES..]);
                    found |= self
                        .verify_inner(
                            &discharges[d_idx],
                            &mut inner_key,
                            tm,
                            discharges,
                            depth + 1,
                        )
                        .is_ok();
                    crypto::explicit_bzero(&mut nonce);
                    crypto::explicit_bzero(&mut plaintext);
                    crypto::explicit_bzero(&mut ciphertext);
                }
            }

            let messages = &[
                identifier.as_bytes(),
                &secret.nonce[..NONCE_BYTES],
                &secret.box_bytes,
                &secret.data,
            ];
            crypto::chained_hmac_n(key, messages);
        }
        found && !fail
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::crypto::*;
    use super::*;

    #[test]
    fn usize() {
        assert!(usize::BITS >= u32::BITS)
    }

    const THIRD_PARTY_BYTES: usize = 64;

    #[test]
    fn constants() {
        assert_eq!(
            SIGNATURE_BYTES,
            libsodium_sys::crypto_secretbox_xsalsa20poly1305_KEYBYTES as usize
        );
        assert_eq!(
            SIGNATURE_BYTES,
            libsodium_sys::crypto_auth_hmacsha256_KEYBYTES as usize
        );
        assert!(THIRD_PARTY_BYTES >= SIGNATURE_BYTES + NONCE_BYTES + BOX_ZERO_BYTES - ZERO_BYTES);
        assert!(SIGNATURE_BYTES >= NONCE_BYTES);

        // If changing this, update the struct Caveat::ThirdParty.
        assert_eq!(NONCE_BYTES, 24);
    }

    #[test]
    fn test_secretbox_xsalsa20poly1305() {
        use super::crypto::*;

        let key = [0u8; ZERO_BYTES];
        let nonce = [
            0xca, 0x17, 0x6e, 0x20, 0xce, 0xf4, 0xc0, 0x2b, 0x79, 0xb8, 0xb8, 0x54, 0x18, 0x46,
            0x48, 0xeb, 0x78, 0xb7, 0x93, 0x26, 0x77, 0xb4, 0xa2, 0xb7,
        ];
        let known_plaintext = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
            23, 24, 25, 26, 27, 28, 29, 30, 32, 33,
        ];
        let known_ciphertext = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 221, 249, 124, 108, 109, 242, 210,
            174, 75, 16, 113, 137, 184, 187, 235, 22, 203, 38, 88, 9, 192, 28, 153, 118, 73, 160,
            162, 90, 231, 25, 109, 203, 125, 202, 5, 154, 203, 100, 253, 229, 85, 34, 10, 44, 108,
            180, 112,
        ];
        let mut plaintext = known_plaintext.clone();
        let mut ciphertext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
        // encrypt
        secretbox_xsalsa20poly1305(&key, &nonce, &plaintext, &mut ciphertext);
        assert_eq!(known_ciphertext, ciphertext);
        // decrypt
        let success = secretbox_xsalsa20poly1305_open(&key, &nonce, &ciphertext, &mut plaintext);
        assert!(success);
        assert_eq!(known_plaintext, plaintext);
    }

    const SECRET: Secret = Secret {
        bytes: [
            0x90, 0xcb, 0x0a, 0xc8, 0x5f, 0xd5, 0xad, 0x88, 0x0f, 0xed, 0xf4, 0x4c, 0x2e, 0xac,
            0x8f, 0xad, 0x2b, 0x58, 0x0f, 0xf6, 0x75, 0xad, 0x99, 0xaa, 0x79, 0xd5, 0x09, 0x71,
            0xff, 0x72, 0xa8, 0x13,
        ],
    };

    const SECRET2: Secret = Secret {
        bytes: [
            0xcf, 0xfe, 0x41, 0x33, 0x60, 0x25, 0x7f, 0xe5, 0xbf, 0xe7, 0x13, 0xe3, 0xd8, 0x01,
            0x98, 0x49, 0x3f, 0x46, 0xff, 0x39, 0x82, 0xbe, 0xbb, 0x71, 0xa3, 0xdb, 0x0f, 0x5a,
            0x37, 0xbf, 0x3e, 0xc0,
        ],
    };

    const NONCE: Secret = Secret {
        bytes: [
            77, 7, 89, 54, 102, 186, 68, 35, 238, 87, 239, 15, 145, 99, 66, 233, 155, 109, 210,
            222, 10, 201, 135, 142, 182, 179, 56, 220, 150, 57, 139, 168,
        ],
    };

    fn expect(macaroon: Macaroon, exp: &str) {
        let got = format!("{:#?}", macaroon);
        assert_eq!(got.trim(), exp.trim());
    }

    #[test]
    fn default_macaroon() {
        let macaroon = Macaroon::default();
        expect(
            macaroon,
            "
location: 
identifier: 
signature: 0000000000000000000000000000000000000000000000000000000000000000
0 caveats
",
        )
    }

    #[test]
    fn new_macaroon() {
        let macaroon = Macaroon::new(
            "http://example.org/macaroons".to_owned(),
            "alice@example.org".to_owned(),
            SECRET,
        );
        expect(
            macaroon,
            "
location: http://example.org/macaroons
identifier: alice@example.org
signature: df8d9f90d5f1a6a3c6aa897001ec26a65d09692153eb408547be5fb38ac9085b
0 caveats
",
        )
    }

    mod caveats {
        use super::*;

        #[test]
        fn exact_string() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            );
            macaroon.add_exact_string("ip = 127.0.0.1".to_owned());
            expect(
                macaroon,
                "
location: http://example.org/macaroons
identifier: alice@example.org
signature: 6c85bc5fc9158431cf2e6fce5fc9a07f859e866e6ac8bdceed3b342b34f2fd86
1 caveat
exact-string: what=\"ip = 127.0.0.1\"
",
            )
        }

        #[test]
        fn expires() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            );
            macaroon.add_expires(1693945396);
            expect(
                macaroon,
                "
location: http://example.org/macaroons
identifier: alice@example.org
signature: 46ba0d6677fdf7be67167f279eb41a3471f7b899e7cf5542ff9bbdd0bbed2f02
1 caveat
expires: when=1693945396
",
            )
        }

        #[test]
        fn third_party() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            );
            let tps = ThirdPartySecret::new(&macaroon.signature, &NONCE, &SECRET2);
            macaroon.add_third_party_caveat(
                "http://example.net/auth".to_owned(),
                "bob@example.net".to_owned(),
                tps,
            );
            expect(
                macaroon,
                "
location: http://example.org/macaroons
identifier: alice@example.org
signature: f9933abf18be79303ff46a0cf1ab5eb15c886932303f7c2b2217b734c97d79d1
1 caveat
third-party: location=http://example.net/auth identifier=bob@example.net
",
            )
        }
    }

    mod verifier {
        use super::*;

        #[test]
        fn exact_string() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            );
            macaroon.add_exact_string("ip = 127.0.0.1".to_owned());
            let mut verifier = Verifier::default();
            verifier.add_context("ip = 127.0.0.1".to_owned());
            assert!(verifier.verify(&macaroon, &SECRET, &[]).is_ok());
        }

        #[test]
        fn expires() {
            const NOW: u64 = 1693945396;
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            );
            macaroon.add_expires(NOW);
            let mut verifier = Verifier::default();
            verifier.set_current_time(NOW);
            assert!(verifier.verify(&macaroon, &SECRET, &[]).is_err());
            verifier.set_current_time(NOW - 1);
            assert!(verifier.verify(&macaroon, &SECRET, &[]).is_ok());
        }

        #[test]
        fn third_party() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            );
            assert_eq!(
                &[
                    0xdf, 0x8d, 0x9f, 0x90, 0xd5, 0xf1, 0xa6, 0xa3, 0xc6, 0xaa, 0x89, 0x70, 0x1,
                    0xec, 0x26, 0xa6, 0x5d, 0x9, 0x69, 0x21, 0x53, 0xeb, 0x40, 0x85, 0x47, 0xbe,
                    0x5f, 0xb3, 0x8a, 0xc9, 0x8, 0x5b
                ],
                &macaroon.signature.bytes
            );
            let tps = ThirdPartySecret::new(&macaroon.signature, &NONCE, &SECRET2);
            macaroon.add_third_party_caveat(
                "http://example.net/auth".to_owned(),
                "bob@example.net".to_owned(),
                tps,
            );
            let mut discharge = Macaroon::new("http://example.net/auth".to_owned(), "bob@example.net".to_owned(), SECRET2);
            macaroon.bind_discharge(&mut discharge);
            let verifier = Verifier::default();
            assert!(verifier.verify(&macaroon, &SECRET, &[]).is_err());
            assert!(verifier.verify(&macaroon, &SECRET, &[discharge]).is_ok());
        }
    }
}
