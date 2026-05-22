#![doc = include_str!("../README.md")]

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::c_void;
use std::fmt::Debug;

use biometrics::Counter;

use buffertk::{Unpacker, stack_pack};
use prototk::Error as PrototkError;

use prototk_derive::Message;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

/// The number of bytes expected in a macaroon signature.
pub const SIGNATURE_BYTES: usize = 32;

/// The number of bytes expected in a third-party secretbox nonce.
pub const NONCE_BYTES: usize = libsodium_sys::crypto_secretbox_xsalsa20poly1305_NONCEBYTES as usize;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static VERIFIER_FIRST_PARTY_FAILURE: Counter = Counter::new("macaroons.verifier.1st_party_failure");
static VERIFIER_FIRST_PARTY_SUCCESS: Counter = Counter::new("macaroons.verifier.1st_party_success");
static VERIFIER_THIRD_PARTY_FAILURE: Counter = Counter::new("macaroons.verifier.3rd_party_failure");
static VERIFIER_THIRD_PARTY_SUCCESS: Counter = Counter::new("macaroons.verifier.3rd_party_success");

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// The error cases macarunes can encounter.
#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
pub enum Error {
    /// Success is the default error.
    #[prototk(294912, message)]
    #[default]
    Success,
    /// The macarunes detected a cycle.  This is a P(0) event for valid macarunes trees.
    #[prototk(294913, message)]
    Cycle,
    /// The proof carried by macarunes does not hold up under scrutiny.
    #[prototk(294914, message)]
    ProofInvalid,
    /// The client is lacking a loader for the given macarune.
    #[prototk(294915, message)]
    MissingLoader {
        /// A textual description of the loader that's needed.
        #[prototk(1, string)]
        what: String,
    },
    /// The loader returned a macarune for a different location than the one requested.
    #[prototk(294916, message)]
    LocationMismatch {
        /// The location that was requested from the loader.
        #[prototk(1, string)]
        expected: String,
        /// The location returned by the loader.
        #[prototk(2, string)]
        actual: String,
    },
    /// A covering set could not find a macaroon with the required public location and identifier.
    #[prototk(294917, message)]
    MissingMacaroon {
        /// The public location required by a third-party caveat.
        #[prototk(1, string)]
        location: String,
        /// The public identifier required by a third-party caveat.
        #[prototk(2, string)]
        identifier: String,
    },
    /// A request builder already has a loader for this location.
    #[prototk(294918, message)]
    DuplicateLoader {
        /// The duplicate static loader location.
        #[prototk(1, string)]
        location: String,
    },
    /// Encoded macaroon bytes could not be decoded as exactly one macaroon.
    #[prototk(294919, message)]
    InvalidEncoding {
        /// A diagnostic description of the decoding problem.
        #[prototk(1, string)]
        what: String,
    },
    /// Secure random secret generation failed.
    #[prototk(294920, message)]
    RandomGenerationFailed,
    /// Third-party secret encryption failed.
    #[prototk(294921, message)]
    EncryptionFailed,
    /// A required cryptographic operation failed.
    #[prototk(294922, message)]
    CryptoOperationFailed,
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Error::Success => write!(fmt, "success"),
            Error::Cycle => write!(fmt, "cycle detected while verifying macaroon discharges"),
            Error::ProofInvalid => write!(fmt, "macaroon proof is invalid"),
            Error::MissingLoader { what } => {
                write!(fmt, "missing macaroon loader for {what:?}")
            }
            Error::LocationMismatch { expected, actual } => write!(
                fmt,
                "loader returned macaroon for {actual:?}, expected {expected:?}"
            ),
            Error::MissingMacaroon {
                location,
                identifier,
            } => write!(
                fmt,
                "missing macaroon for location {location:?} and identifier {identifier:?}"
            ),
            Error::DuplicateLoader { location } => {
                write!(fmt, "duplicate macaroon loader for {location:?}")
            }
            Error::InvalidEncoding { what } => {
                write!(fmt, "invalid macaroon encoding: {what}")
            }
            Error::RandomGenerationFailed => write!(fmt, "secure random generation failed"),
            Error::EncryptionFailed => write!(fmt, "third-party secret encryption failed"),
            Error::CryptoOperationFailed => write!(fmt, "cryptographic operation failed"),
        }
    }
}

impl std::error::Error for Error {}

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
    #[prototk(3, message)]
    NotBefore {
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
            Caveat::NotBefore { .. } => true,
            Caveat::ThirdParty { .. } => false,
        }
    }

    #[cfg(test)]
    pub fn is_third_party(&self) -> bool {
        match self {
            Caveat::ExactString { .. } => false,
            Caveat::Expires { .. } => false,
            Caveat::NotBefore { .. } => false,
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
                write!(fmt, "exact-string: what={what:#?}")?;
            }
            Caveat::Expires { when } => {
                write!(fmt, "expires: when={when}")?;
            }
            Caveat::NotBefore { when } => {
                write!(fmt, "not-before: when={when}")?;
            }
            Caveat::ThirdParty {
                location,
                identifier,
                ..
            } => {
                write!(
                    fmt,
                    "third-party: location={location} identifier={identifier}"
                )?;
            }
        }
        Ok(())
    }
}

/// A read-only view of one macaroon caveat.
///
/// Third-party caveats expose only their public routing hint and identifier.  The encrypted
/// [ThirdPartySecret] remains private to the macaroon implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaveatRef<'a> {
    /// A first-party exact-string caveat.
    ExactString {
        /// The exact context string required by the caveat.
        what: &'a str,
    },
    /// A first-party expiration caveat.
    Expires {
        /// The exclusive expiration timestamp.
        when: u64,
    },
    /// A first-party not-before caveat.
    NotBefore {
        /// The inclusive lower-bound timestamp.
        when: u64,
    },
    /// A third-party discharge requirement.
    ThirdParty {
        /// The untrusted routing hint for finding a discharge mechanism.
        location: &'a str,
        /// The public identifier to present to the discharge mechanism.
        identifier: &'a str,
    },
}

impl<'a> CaveatRef<'a> {
    fn from_caveat(caveat: &'a Caveat) -> Self {
        match caveat {
            Caveat::ExactString { what } => Self::ExactString { what },
            Caveat::Expires { when } => Self::Expires { when: *when },
            Caveat::NotBefore { when } => Self::NotBefore { when: *when },
            Caveat::ThirdParty {
                location,
                identifier,
                ..
            } => Self::ThirdParty {
                location,
                identifier,
            },
        }
    }
}

////////////////////////////////////////////// crypto //////////////////////////////////////////////

mod crypto {
    use super::*;
    use std::sync::Once;
    use std::sync::atomic::{AtomicBool, Ordering};

    static SODIUM_INIT: Once = Once::new();
    static SODIUM_INIT_OK: AtomicBool = AtomicBool::new(false);

    pub const ZERO_BYTES: usize =
        libsodium_sys::crypto_secretbox_xsalsa20poly1305_ZEROBYTES as usize;
    pub const BOX_ZERO_BYTES: usize =
        libsodium_sys::crypto_secretbox_xsalsa20poly1305_BOXZEROBYTES as usize;

    /// Ensure libsodium is initialized before using any of its APIs.
    pub fn ensure_initialized() -> bool {
        SODIUM_INIT.call_once(|| {
            // SAFETY(codex): `sodium_init` has no caller-provided pointers or buffers.  It is designed to
            // be called before any other libsodium API and may be called more than once; `Once`
            // keeps that initialization path single-shot for this process.
            let ret = unsafe { libsodium_sys::sodium_init() };
            SODIUM_INIT_OK.store(ret >= 0, Ordering::Relaxed);
        });
        SODIUM_INIT_OK.load(Ordering::Relaxed)
    }

    /// hmac is not public.  Everything should use a wrapper in this module.
    fn hmac(key: &Secret, message: &[u8], out: &mut Secret) -> bool {
        if !ensure_initialized() {
            return false;
        }
        // SAFETY(codex): `out` points at a writable 32-byte `Secret`; `key` points at a readable 32-byte
        // HMAC key; and `message.as_ptr()` is valid for `message.len()` bytes.  Callers use a
        // separate `Secret` for output, so the output buffer does not alias the input key.
        unsafe {
            libsodium_sys::crypto_auth_hmacsha256(
                out.bytes.as_mut_ptr(),
                message.as_ptr(),
                message.len() as u64,
                key.bytes.as_ptr(),
            );
        }
        true
    }

    pub fn explicit_bzero(bytes: &mut [u8]) {
        let _ = ensure_initialized();
        // SAFETY(codex): `bytes.as_mut_ptr()` is valid for `bytes.len()` bytes and may be passed to
        // libsodium's memory clearing routine.  Empty slices still provide a non-null, aligned
        // pointer that is valid for a zero-length operation.
        unsafe {
            libsodium_sys::sodium_memzero(bytes.as_mut_ptr() as *mut c_void, bytes.len());
        }
    }

    pub fn random_bytes(bytes: &mut [u8]) -> bool {
        if !ensure_initialized() {
            explicit_bzero(bytes);
            return false;
        }
        // SAFETY(codex): `bytes.as_mut_ptr()` is valid for `bytes.len()` writable bytes.
        unsafe {
            libsodium_sys::randombytes_buf(bytes.as_mut_ptr() as *mut c_void, bytes.len());
        }
        true
    }

    pub fn mem_eq(lhs: &[u8], rhs: &[u8]) -> bool {
        if !ensure_initialized() {
            return false;
        }
        // SAFETY(codex): Both slice pointers are valid for at least `min(lhs.len(), rhs.len())` readable
        // bytes.  The length equality check happens separately so that unequal lengths cannot be
        // accepted after a shared prefix compares equal.
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

    pub fn chained_hmac_1(key: &mut Secret, message: &[u8]) -> bool {
        let orig = Secret { bytes: key.bytes };
        let ok = hmac(&orig, message, key);
        if !ok {
            key.scrub();
        }
        ok
    }

    pub fn chained_hmac_n(key: &mut Secret, messages: &[&[u8]]) -> bool {
        for message in messages {
            if !chained_hmac_1(key, message) {
                return false;
            }
        }
        true
    }

    pub fn bind_for_request(root: &Macaroon, sig: &mut Secret) -> bool {
        chained_hmac_1(sig, &root.signature.bytes)
    }

    pub fn secretbox_xsalsa20poly1305(
        key: &[u8; SIGNATURE_BYTES],
        nonce: &[u8; NONCE_BYTES],
        plaintext: &[u8; ZERO_BYTES + SIGNATURE_BYTES],
        ciphertext: &mut [u8; ZERO_BYTES + SIGNATURE_BYTES],
    ) -> bool {
        if !ensure_initialized() {
            explicit_bzero(ciphertext);
            return false;
        }
        for p in &plaintext[0..ZERO_BYTES] {
            if *p != 0 {
                explicit_bzero(ciphertext);
                return false;
            }
        }
        // SAFETY(codex): `plaintext`, `ciphertext`, `nonce`, and `key` are fixed-size arrays whose sizes
        // match the legacy secretbox API.  The input and output arrays are distinct, writable/readable
        // for the declared length, and the required leading zero bytes have been checked above.
        let ret = unsafe {
            libsodium_sys::crypto_secretbox_xsalsa20poly1305(
                ciphertext.as_mut_ptr(),
                plaintext.as_ptr(),
                (ZERO_BYTES + SIGNATURE_BYTES) as u64,
                nonce.as_ptr(),
                key.as_ptr(),
            )
        };
        ret == 0
    }

    #[must_use]
    pub fn secretbox_xsalsa20poly1305_open(
        key: &[u8; SIGNATURE_BYTES],
        nonce: &[u8; NONCE_BYTES],
        ciphertext: &[u8; ZERO_BYTES + SIGNATURE_BYTES],
        plaintext: &mut [u8; ZERO_BYTES + SIGNATURE_BYTES],
    ) -> bool {
        if !ensure_initialized() {
            explicit_bzero(plaintext);
            return false;
        }
        for c in &ciphertext[0..BOX_ZERO_BYTES] {
            if *c != 0 {
                explicit_bzero(plaintext);
                return false;
            }
        }
        // SAFETY(codex): `ciphertext`, `plaintext`, `nonce`, and `key` are fixed-size arrays whose sizes
        // match the legacy secretbox-open API.  The input and output arrays are distinct,
        // writable/readable for the declared length, and the required leading zero bytes have been
        // checked above.
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

/////////////////////////////////////////////// Nonce //////////////////////////////////////////////

/// A public nonce for wrapping a [ThirdPartySecret].
///
/// A nonce is not secret, but it must be unique for a given macaroon signature.  Prefer
/// [Nonce::random] for production use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Nonce {
    bytes: [u8; NONCE_BYTES],
}

impl Nonce {
    /// Construct a nonce from raw bytes.
    ///
    /// This is primarily useful for deterministic tests and for loading nonce material that was
    /// generated elsewhere.
    pub const fn from_bytes(bytes: [u8; NONCE_BYTES]) -> Self {
        Self { bytes }
    }

    /// Generate a random nonce using a secure random number generator.
    pub fn random() -> Result<Self, Error> {
        let mut bytes = [0u8; NONCE_BYTES];
        if crypto::random_bytes(&mut bytes) {
            Ok(Self { bytes })
        } else {
            Err(Error::RandomGenerationFailed)
        }
    }

    /// Return the raw nonce bytes.
    pub fn as_bytes(&self) -> &[u8; NONCE_BYTES] {
        &self.bytes
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
    /// Construct a [Secret] from raw bytes.
    pub fn from_bytes(bytes: [u8; SIGNATURE_BYTES]) -> Self {
        Self { bytes }
    }

    /// A textual representation of the secret.
    ///
    /// This exposes the underlying key material.  Use it only for controlled diagnostics and tests;
    /// never log it for live credentials.
    pub fn hexdigest(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut hexdigest = String::with_capacity(2 * SIGNATURE_BYTES);
        for item in &self.bytes {
            let item = *item;
            hexdigest.push(HEX[(item >> 4) as usize] as char);
            hexdigest.push(HEX[(item & 0x0f) as usize] as char);
        }
        hexdigest
    }

    /// Do our best to scrub the secret from memory.
    pub fn scrub(&mut self) {
        crypto::explicit_bzero(&mut self.bytes);
    }

    /// Generate a random secret using a secure random number generator.
    pub fn random() -> Result<Self, Error> {
        let mut x = Self {
            bytes: [0u8; SIGNATURE_BYTES],
        };
        if crypto::random_bytes(&mut x.bytes) {
            Ok(x)
        } else {
            Err(Error::RandomGenerationFailed)
        }
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
    pub fn new(signature: &Secret, nonce: Nonce, discharge_secret: &Secret) -> Result<Self, Error> {
        use crypto::{BOX_ZERO_BYTES, ZERO_BYTES};
        let mut tps = ThirdPartySecret::default();
        let mut nonce_bytes = [0u8; NONCE_BYTES];
        nonce_bytes.copy_from_slice(nonce.as_bytes());
        let mut plaintext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
        plaintext[ZERO_BYTES..].copy_from_slice(&discharge_secret.bytes);
        let mut ciphertext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
        let encrypted = crypto::secretbox_xsalsa20poly1305(
            &signature.bytes,
            &nonce_bytes,
            &plaintext,
            &mut ciphertext,
        );
        if encrypted {
            tps.nonce[0..NONCE_BYTES].copy_from_slice(&nonce_bytes);
            tps.nonce[NONCE_BYTES..].copy_from_slice(&[0u8; 8]);
            tps.box_bytes.copy_from_slice(
                &ciphertext[BOX_ZERO_BYTES..BOX_ZERO_BYTES + (ZERO_BYTES - BOX_ZERO_BYTES)],
            );
            tps.data.copy_from_slice(&ciphertext[ZERO_BYTES..]);
        }
        crypto::explicit_bzero(&mut nonce_bytes);
        crypto::explicit_bzero(&mut plaintext);
        crypto::explicit_bzero(&mut ciphertext);
        if encrypted {
            Ok(tps)
        } else {
            Err(Error::EncryptionFailed)
        }
    }

    /// Create a new [ThirdPartySecret] with a fresh nonce.
    pub fn random(signature: &Secret, discharge_secret: &Secret) -> Result<Self, Error> {
        let nonce = Nonce::random()?;
        Self::new(signature, nonce, discharge_secret)
    }

    /// Scrub the memory locations of the nonce and ciphertext.
    pub fn scrub(&mut self) {
        crypto::explicit_bzero(&mut self.nonce);
        crypto::explicit_bzero(&mut self.box_bytes);
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
    pub fn new(
        location: impl Into<String>,
        identifier: impl Into<String>,
        mut secret: Secret,
    ) -> Result<Self, Error> {
        let identifier = identifier.into();
        if !crypto::chained_hmac_1(&mut secret, identifier.as_bytes()) {
            return Err(Error::CryptoOperationFailed);
        }
        Ok(Macaroon {
            location: location.into(),
            identifier,
            signature: secret,
            caveats: Vec::new(),
        })
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

    /// Encode this macaroon to its prototk byte representation.
    pub fn to_bytes(&self) -> Vec<u8> {
        stack_pack(self).to_vec()
    }

    /// Decode exactly one macaroon from its prototk byte representation.
    ///
    /// Trailing bytes are rejected.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut unpacker = Unpacker::new(bytes);
        let macaroon: Macaroon =
            unpacker
                .unpack()
                .map_err(|error: PrototkError| Error::InvalidEncoding {
                    what: error.to_string(),
                })?;
        let canonical = macaroon.to_bytes();
        if bytes == canonical {
            Ok(macaroon)
        } else if bytes.starts_with(&canonical) && bytes.len() > canonical.len() {
            Err(Error::InvalidEncoding {
                what: format!("{} trailing bytes", bytes.len() - canonical.len()),
            })
        } else {
            Err(Error::InvalidEncoding {
                what: "non-canonical macaroon encoding".to_owned(),
            })
        }
    }

    /// The number of caveats attached to the macaroon.
    pub fn caveat_count(&self) -> usize {
        self.caveats.len()
    }

    /// Iterate over read-only caveat views.
    pub fn caveats(
        &self,
    ) -> impl ExactSizeIterator<Item = CaveatRef<'_>> + DoubleEndedIterator + '_ {
        self.caveats.iter().map(CaveatRef::from_caveat)
    }

    /// Returns true if this macaroon has any caveats.
    pub fn has_caveats(&self) -> bool {
        !self.caveats.is_empty()
    }

    /// Add a caveat that must match exactly.
    pub fn add_exact_string(&mut self, what: impl Into<String>) -> Result<(), Error> {
        self.add_caveat(Caveat::ExactString { what: what.into() })
    }

    /// Add a caveat that expires the macaroon after when.
    pub fn add_expires(&mut self, when: u64) -> Result<(), Error> {
        self.add_caveat(Caveat::Expires { when })
    }

    /// Add a caveat that rejects verifier times before `when`.
    pub fn add_not_before(&mut self, when: u64) -> Result<(), Error> {
        self.add_caveat(Caveat::NotBefore { when })
    }

    /// Add a third party caveat.  Provide `signature()` to ask the third party to generate the
    /// identifier and secret.
    ///
    /// The location is an unsigned routing hint.  Verification depends on the identifier,
    /// encrypted verification-key material, and signature chain.
    pub fn add_third_party_caveat(
        &mut self,
        location: impl Into<String>,
        identifier: impl Into<String>,
        secret: ThirdPartySecret,
    ) -> Result<(), Error> {
        self.add_caveat(Caveat::ThirdParty {
            location: location.into(),
            identifier: identifier.into(),
            secret,
        })
    }

    /// Bind a macaroon to the request to make sure discharge macaroons cannot be used in other
    /// contexts.
    pub fn bind_discharge(&self, discharge: &mut Macaroon) -> Result<(), Error> {
        let mut signature = discharge.signature.clone();
        if !crypto::bind_for_request(self, &mut signature) {
            return Err(Error::CryptoOperationFailed);
        }
        discharge.signature = signature;
        Ok(())
    }

    /// Assemble the transitive discharge cover from `candidates`.
    ///
    /// The returned set contains discharge macaroons only, not `self`.  Selection is based only on
    /// public macaroon data: locations, identifiers, and third-party caveat references.  This does
    /// not verify signatures, decrypt third-party secrets, or prove that the selected macaroons
    /// satisfy their caveats; pass the returned set to [Verifier::verify] for that.
    ///
    /// If more than one candidate has the same public location and identifier, the first candidate
    /// is selected because public data cannot distinguish which proof is valid.
    pub fn covering_set(&self, candidates: &[Macaroon]) -> Result<Vec<Macaroon>, Error> {
        Ok(self
            .covering_set_refs(candidates)?
            .into_iter()
            .cloned()
            .collect())
    }

    /// Assemble the transitive discharge cover from `candidates`, returning references into the
    /// candidate slice.
    ///
    /// See [Macaroon::covering_set] for selection semantics.
    pub fn covering_set_refs<'a>(
        &self,
        candidates: &'a [Macaroon],
    ) -> Result<Vec<&'a Macaroon>, Error> {
        let mut cover = Vec::new();
        let mut queue = VecDeque::new();
        let mut enqueued = HashSet::new();

        self.enqueue_third_party_requirements(&mut queue, &mut enqueued);

        while let Some((location, identifier)) = queue.pop_front() {
            let discharge = candidates
                .iter()
                .find(|candidate| {
                    crypto::str_eq(candidate.location(), &location)
                        && crypto::str_eq(candidate.identifier(), &identifier)
                })
                .ok_or_else(|| Error::MissingMacaroon {
                    location: location.clone(),
                    identifier: identifier.clone(),
                })?;
            discharge.enqueue_third_party_requirements(&mut queue, &mut enqueued);
            cover.push(discharge);
        }

        Ok(cover)
    }

    fn enqueue_third_party_requirements(
        &self,
        queue: &mut VecDeque<(String, String)>,
        enqueued: &mut HashSet<(String, String)>,
    ) {
        for caveat in &self.caveats {
            if let Caveat::ThirdParty {
                location,
                identifier,
                ..
            } = caveat
            {
                let key = (location.clone(), identifier.clone());
                if enqueued.insert(key.clone()) {
                    queue.push_back(key);
                }
            }
        }
    }

    fn add_caveat(&mut self, caveat: Caveat) -> Result<(), Error> {
        match &caveat {
            Caveat::ExactString { .. } | Caveat::Expires { .. } | Caveat::NotBefore { .. } => {
                let buf = stack_pack(&caveat).to_vec();
                let mut signature = self.signature.clone();
                if !crypto::chained_hmac_1(&mut signature, &buf) {
                    return Err(Error::CryptoOperationFailed);
                }
                self.caveats.push(caveat);
                self.signature = signature;
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
                let mut signature = self.signature.clone();
                if !crypto::chained_hmac_n(&mut signature, messages) {
                    return Err(Error::CryptoOperationFailed);
                }
                self.caveats.push(caveat);
                self.signature = signature;
            }
        }
        Ok(())
    }
}

impl Debug for Macaroon {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        writeln!(fmt, "location: {}", self.location)?;
        writeln!(fmt, "identifier: {}", self.identifier)?;
        writeln!(fmt, "signature: [redacted]")?;
        if self.caveats.len() == 1 {
            writeln!(fmt, "1 caveat")?;
        } else {
            writeln!(fmt, "{} caveats", self.caveats.len())?;
        }
        for caveat in &self.caveats {
            writeln!(fmt, "{caveat:?}")?;
        }
        Ok(())
    }
}

////////////////////////////////////////////// Loader //////////////////////////////////////////////

/// A [Loader] is a client-side way to get macaroons by identifier.  Used by [RequestBuilder].
pub trait Loader: Debug {
    /// The static location this loader serves.
    ///
    /// Static loader locations are trusted application code, not authority derived from macaroon
    /// data.
    fn location(&self) -> &'static str;
    /// Lookup the discharge macaroon with the given identifier.
    ///
    /// Assumes the caller will verify the macaroon's location against `self.location()`.
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

    /// Add a loader for macaroons.
    ///
    /// Every type of third party should have its own loader identified by its static
    /// [Loader::location].  Duplicate locations are rejected so untrusted macaroon data cannot
    /// replace code-registered discharge mechanisms.
    pub fn add_loader<L: Loader + 'static>(&mut self, loader: L) -> Result<(), Error> {
        let location = loader.location();
        if self.loaders.contains_key(location) {
            return Err(Error::DuplicateLoader {
                location: location.to_owned(),
            });
        }
        self.loaders.insert(location, Box::new(loader));
        Ok(())
    }

    /// Add a loader and return the builder for call chaining.
    pub fn with_loader<L: Loader + 'static>(mut self, loader: L) -> Result<Self, Error> {
        self.add_loader(loader)?;
        Ok(self)
    }

    /// Prepare a request with the location and identifier provided.
    pub fn prepare_request(
        &self,
        location: &str,
        identifier: &str,
    ) -> Result<(Macaroon, Vec<Macaroon>), Error> {
        let root = self.lookup_macaroon(location, identifier)?;
        let mut discharges = Vec::new();
        let mut queue = VecDeque::new();
        let mut enqueued = HashSet::new();
        self.enqueue_discharges(&root, &mut discharges, &mut queue, &mut enqueued)?;
        while let Some(index) = queue.pop_front() {
            let macaroon = discharges[index].clone();
            self.enqueue_discharges(&macaroon, &mut discharges, &mut queue, &mut enqueued)?;
        }
        for discharge in &mut discharges {
            root.bind_discharge(discharge)?;
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

    fn lookup_macaroon(&self, location: &str, identifier: &str) -> Result<Macaroon, Error> {
        let macaroon = self.get_loader_for(location)?.lookup(identifier)?;
        if crypto::str_eq(macaroon.location(), location) {
            Ok(macaroon)
        } else {
            Err(Error::LocationMismatch {
                expected: location.to_owned(),
                actual: macaroon.location().to_owned(),
            })
        }
    }

    fn enqueue_discharges(
        &self,
        macaroon: &Macaroon,
        discharges: &mut Vec<Macaroon>,
        queue: &mut VecDeque<usize>,
        enqueued: &mut HashSet<(String, String)>,
    ) -> Result<(), Error> {
        for caveat in macaroon.caveats.iter() {
            if let Caveat::ThirdParty {
                location,
                identifier,
                secret: _,
            } = caveat
            {
                let key = (location.clone(), identifier.clone());
                if enqueued.insert(key) {
                    let discharge = self.lookup_macaroon(location, identifier)?;
                    queue.push_back(discharges.len());
                    discharges.push(discharge);
                }
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
    now: Option<u64>,
    contexts: Vec<String>,
}

impl Verifier {
    /// Create a new verifier.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `context` to the context of the request.  Concretely adding a string here means the
    /// client can add a contextual caveat that we will verify.
    pub fn add_context(&mut self, context: impl Into<String>) {
        self.contexts.push(context.into());
    }

    /// Add `context` and return the verifier for call chaining.
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.add_context(context);
        self
    }

    /// Set the time to use for the verifier.
    pub fn set_current_time(&mut self, now: u64) {
        self.now = Some(now);
    }

    /// Set the time to use and return the verifier for call chaining.
    pub fn with_current_time(mut self, now: u64) -> Self {
        self.set_current_time(now);
        self
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
        if !crypto::chained_hmac_1(secret, m.identifier.as_bytes()) {
            return Err(Error::ProofInvalid);
        }
        let mut fail = false;

        for caveat in &m.caveats {
            let (caveat_ok, success_counter, failure_counter) = match caveat {
                Caveat::ExactString { .. } | Caveat::Expires { .. } | Caveat::NotBefore { .. } => {
                    let caveat_ok = self.verify_1st(secret, caveat);
                    (
                        caveat_ok,
                        &VERIFIER_FIRST_PARTY_SUCCESS,
                        &VERIFIER_FIRST_PARTY_FAILURE,
                    )
                }
                Caveat::ThirdParty { .. } => {
                    let caveat_ok = self.verify_3rd(secret, caveat, tm, discharges, depth);
                    (
                        caveat_ok,
                        &VERIFIER_THIRD_PARTY_SUCCESS,
                        &VERIFIER_THIRD_PARTY_FAILURE,
                    )
                }
            };
            if caveat_ok {
                success_counter.click();
            } else {
                failure_counter.click();
                fail = true;
            }
        }

        if m != tm {
            fail |= !crypto::bind_for_request(tm, &mut *secret);
        }

        fail |= *secret != m.signature;

        if fail {
            Err(Error::ProofInvalid)
        } else {
            Ok(())
        }
    }

    fn verify_1st(&self, secret: &mut Secret, caveat: &Caveat) -> bool {
        if !caveat.is_first_party() {
            return false;
        }
        let mut found = false;
        let buf = stack_pack(caveat).to_vec();
        if !crypto::chained_hmac_1(secret, &buf) {
            return false;
        }
        match caveat {
            Caveat::ExactString { what } => {
                for context in &self.contexts {
                    found |= crypto::mem_eq(context.as_bytes(), what.as_bytes());
                }
            }
            Caveat::Expires { when } => {
                found |= self.now.map(|now| *when > now).unwrap_or(false);
            }
            Caveat::NotBefore { when } => {
                found |= self.now.map(|now| now >= *when).unwrap_or(false);
            }
            Caveat::ThirdParty { .. } => return false,
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
        use crypto::{BOX_ZERO_BYTES, ZERO_BYTES};
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
                &secret.nonce,
                &secret.box_bytes,
                &secret.data,
            ];
            fail |= !crypto::chained_hmac_n(key, messages);
        }
        found && !fail
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::crypto::*;
    use super::*;
    use buffertk::Unpacker;

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn usize() {
        assert!(usize::BITS >= u32::BITS)
    }

    const THIRD_PARTY_BYTES: usize = 64;

    #[allow(clippy::assertions_on_constants)]
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
    fn crypto_wrappers_initialize_libsodium() {
        assert!(ensure_initialized());

        let mut random = [0u8; SIGNATURE_BYTES];
        assert!(random_bytes(&mut random));
        assert!(mem_eq(&random, &random));
    }

    #[test]
    fn secretbox_xsalsa20poly1305_known_vector_round_trips() {
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
        let mut plaintext = known_plaintext;
        let mut ciphertext = [0u8; ZERO_BYTES + SIGNATURE_BYTES];
        // encrypt
        assert!(secretbox_xsalsa20poly1305(
            &key,
            &nonce,
            &plaintext,
            &mut ciphertext
        ));
        assert_eq!(known_ciphertext, ciphertext);
        // decrypt
        let success = secretbox_xsalsa20poly1305_open(&key, &nonce, &ciphertext, &mut plaintext);
        assert!(success);
        assert_eq!(known_plaintext, plaintext);
    }

    #[test]
    fn chained_hmac_n_uses_every_message() {
        let mut baseline = SECRET;
        assert!(chained_hmac_n(
            &mut baseline,
            &[b"identifier", b"nonce", b"box", b"data"]
        ));

        let mut first_changed = SECRET;
        assert!(chained_hmac_n(
            &mut first_changed,
            &[b"other-identifier", b"nonce", b"box", b"data"],
        ));
        let mut middle_changed = SECRET;
        assert!(chained_hmac_n(
            &mut middle_changed,
            &[b"identifier", b"other-nonce", b"box", b"data"],
        ));
        let mut last_changed = SECRET;
        assert!(chained_hmac_n(
            &mut last_changed,
            &[b"identifier", b"nonce", b"box", b"other-data"],
        ));

        assert_ne!(baseline.bytes, first_changed.bytes);
        assert_ne!(baseline.bytes, middle_changed.bytes);
        assert_ne!(baseline.bytes, last_changed.bytes);
    }

    #[test]
    fn chained_hmac_n_preserves_message_order() {
        let mut ordered = SECRET;
        assert!(chained_hmac_n(
            &mut ordered,
            &[b"identifier", b"nonce", b"box", b"data"]
        ));
        let mut reordered = SECRET;
        assert!(chained_hmac_n(
            &mut reordered,
            &[b"data", b"box", b"nonce", b"identifier"]
        ));

        assert_ne!(ordered.bytes, reordered.bytes);
    }

    #[test]
    fn error_display_messages_are_actionable() {
        assert_eq!("success", Error::Success.to_string());
        assert_eq!(
            "cycle detected while verifying macaroon discharges",
            Error::Cycle.to_string()
        );
        assert_eq!("macaroon proof is invalid", Error::ProofInvalid.to_string());
        assert_eq!(
            "missing macaroon loader for \"http://example.net/auth\"",
            Error::MissingLoader {
                what: AUTH_LOCATION.to_owned()
            }
            .to_string()
        );
        assert_eq!(
            "loader returned macaroon for \"actual\", expected \"expected\"",
            Error::LocationMismatch {
                expected: "expected".to_owned(),
                actual: "actual".to_owned(),
            }
            .to_string()
        );
        assert_eq!(
            "missing macaroon for location \"http://example.net/auth\" and identifier \"bob@example.net\"",
            Error::MissingMacaroon {
                location: AUTH_LOCATION.to_owned(),
                identifier: AUTH_IDENTIFIER.to_owned(),
            }
            .to_string()
        );
        assert_eq!(
            "duplicate macaroon loader for \"http://example.net/auth\"",
            Error::DuplicateLoader {
                location: AUTH_LOCATION.to_owned(),
            }
            .to_string()
        );
        assert_eq!(
            "invalid macaroon encoding: 1 trailing bytes",
            Error::InvalidEncoding {
                what: "1 trailing bytes".to_owned(),
            }
            .to_string()
        );
        assert_eq!(
            "secure random generation failed",
            Error::RandomGenerationFailed.to_string()
        );
        assert_eq!(
            "third-party secret encryption failed",
            Error::EncryptionFailed.to_string()
        );
        assert_eq!(
            "cryptographic operation failed",
            Error::CryptoOperationFailed.to_string()
        );
    }

    #[test]
    fn error_round_trips_through_prototk() {
        let cases = [
            Error::Success,
            Error::Cycle,
            Error::ProofInvalid,
            Error::MissingLoader {
                what: AUTH_LOCATION.to_owned(),
            },
            Error::LocationMismatch {
                expected: ROOT_LOCATION.to_owned(),
                actual: AUTH_LOCATION.to_owned(),
            },
            Error::MissingMacaroon {
                location: AUTH_LOCATION.to_owned(),
                identifier: AUTH_IDENTIFIER.to_owned(),
            },
            Error::DuplicateLoader {
                location: AUTH_LOCATION.to_owned(),
            },
            Error::InvalidEncoding {
                what: "1 trailing bytes".to_owned(),
            },
            Error::RandomGenerationFailed,
            Error::EncryptionFailed,
            Error::CryptoOperationFailed,
        ];

        for expected in cases {
            let buf = stack_pack(&expected).to_vec();
            let mut unpacker = Unpacker::new(&buf);
            let got: Error = unpacker.unpack().unwrap();
            assert_eq!(expected, got);
            assert!(unpacker.is_empty());
        }
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

    const SECRET3: Secret = Secret {
        bytes: [
            0x1c, 0x22, 0x27, 0xe1, 0x3a, 0x3f, 0x40, 0x88, 0x16, 0x9d, 0xe4, 0x1b, 0xad, 0xd7,
            0x35, 0x02, 0x5f, 0x41, 0x2d, 0x08, 0x4d, 0x0f, 0x4f, 0xde, 0x1b, 0xa9, 0x74, 0xa7,
            0x3b, 0xcf, 0x3f, 0x82,
        ],
    };

    const NONCE: Nonce = Nonce::from_bytes([
        77, 7, 89, 54, 102, 186, 68, 35, 238, 87, 239, 15, 145, 99, 66, 233, 155, 109, 210, 222,
        10, 201, 135, 142,
    ]);

    const NONCE2: Nonce = Nonce::from_bytes([
        0x4d, 0xd8, 0x47, 0x17, 0xfe, 0x0c, 0x95, 0x31, 0xc1, 0x49, 0x3f, 0x9e, 0xd0, 0x39, 0x49,
        0x8e, 0xb7, 0x92, 0x9d, 0xf5, 0xc8, 0x97, 0xc4, 0x37,
    ]);

    const ROOT_LOCATION: &str = "http://example.org/macaroons";
    const ROOT_IDENTIFIER: &str = "alice@example.org";
    const AUTH_LOCATION: &str = "http://example.net/auth";
    const AUTH_IDENTIFIER: &str = "bob@example.net";
    const NESTED_AUTH_LOCATION: &str = "http://example.com/group-auth";
    const NESTED_AUTH_IDENTIFIER: &str = "carol@example.com";

    fn expect(macaroon: Macaroon, exp: &str) {
        let got = format!("{macaroon:#?}");
        assert_eq!(got.trim(), exp.trim());
    }

    fn root_macaroon() -> Macaroon {
        Macaroon::new(ROOT_LOCATION.to_owned(), ROOT_IDENTIFIER.to_owned(), SECRET).unwrap()
    }

    fn root_with_third_party_caveat() -> Macaroon {
        let mut macaroon = root_macaroon();
        let tps = ThirdPartySecret::new(macaroon.signature(), NONCE, &SECRET2).unwrap();
        macaroon
            .add_third_party_caveat(AUTH_LOCATION.to_owned(), AUTH_IDENTIFIER.to_owned(), tps)
            .unwrap();
        macaroon
    }

    fn discharge_macaroon() -> Macaroon {
        Macaroon::new(
            AUTH_LOCATION.to_owned(),
            AUTH_IDENTIFIER.to_owned(),
            SECRET2,
        )
        .unwrap()
    }

    #[test]
    fn default_macaroon() {
        let macaroon = Macaroon::default();
        expect(
            macaroon,
            "
location: 
identifier: 
signature: [redacted]
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
        )
        .unwrap();
        assert_eq!(
            Macaroon {
                location: ROOT_LOCATION.to_owned(),
                identifier: ROOT_IDENTIFIER.to_owned(),
                signature: Secret {
                    bytes: [
                        0xdf, 0x8d, 0x9f, 0x90, 0xd5, 0xf1, 0xa6, 0xa3, 0xc6, 0xaa, 0x89, 0x70,
                        0x01, 0xec, 0x26, 0xa6, 0x5d, 0x09, 0x69, 0x21, 0x53, 0xeb, 0x40, 0x85,
                        0x47, 0xbe, 0x5f, 0xb3, 0x8a, 0xc9, 0x08, 0x5b,
                    ],
                },
                caveats: Vec::new(),
            },
            macaroon
        );
        expect(
            macaroon,
            "
location: http://example.org/macaroons
identifier: alice@example.org
signature: [redacted]
0 caveats
",
        )
    }

    #[test]
    fn macaroon_debug_redacts_signature() {
        let macaroon = root_with_third_party_caveat();
        let debug = format!("{macaroon:?}");

        assert!(debug.contains("signature: [redacted]"));
        assert!(!debug.contains(&macaroon.signature.hexdigest()));
    }

    #[test]
    fn ergonomic_constructors_accept_borrowed_strings() {
        let mut macaroon = Macaroon::new(ROOT_LOCATION, ROOT_IDENTIFIER, SECRET).unwrap();

        assert_eq!(ROOT_LOCATION, macaroon.location());
        assert_eq!(ROOT_IDENTIFIER, macaroon.identifier());
        assert_eq!(0, macaroon.caveat_count());
        assert!(!macaroon.has_caveats());

        macaroon.add_exact_string("ip = 127.0.0.1").unwrap();

        assert_eq!(1, macaroon.caveat_count());
        assert!(macaroon.has_caveats());
    }

    #[test]
    fn root_location_hint_is_not_signed() {
        let mut macaroon = root_macaroon();
        macaroon.location = "http://mirror.example.org/macaroons".to_owned();

        assert_eq!(Ok(()), Verifier::default().verify(&macaroon, &SECRET, &[]));
    }

    #[test]
    fn root_identifier_is_signed() {
        let mut macaroon = root_macaroon();
        macaroon.identifier = "mallory@example.org".to_owned();

        assert_eq!(
            Err(Error::ProofInvalid),
            Verifier::default().verify(&macaroon, &SECRET, &[])
        );
    }

    #[test]
    fn secret_from_bytes_supports_configured_secrets() {
        let secret = Secret::from_bytes([0xab; SIGNATURE_BYTES]);
        assert_eq!(
            "abababababababababababababababababababababababababababababababab",
            secret.hexdigest()
        );
    }

    #[test]
    fn nonce_from_bytes_preserves_nonce_material() {
        let nonce = Nonce::from_bytes([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
        ]);

        assert_eq!(
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23,
            ],
            nonce.as_bytes()
        );
    }

    #[test]
    fn macaroon_round_trips_through_prototk() {
        let mut expected = root_macaroon();
        expected.add_exact_string("role = admin").unwrap();
        expected.add_not_before(1_800_000_000).unwrap();
        expected.add_expires(1_900_000_000).unwrap();
        let tps = ThirdPartySecret::new(expected.signature(), NONCE, &SECRET2).unwrap();
        expected
            .add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, tps)
            .unwrap();

        let buf = stack_pack(&expected).to_vec();
        let mut unpacker = Unpacker::new(&buf);
        let got: Macaroon = unpacker.unpack().unwrap();

        assert_eq!(expected, got);
        assert!(unpacker.is_empty());
    }

    #[test]
    fn macaroon_to_from_bytes_round_trips() {
        let mut expected = root_macaroon();
        expected.add_exact_string("role = admin").unwrap();
        expected.add_not_before(1_800_000_000).unwrap();
        expected.add_expires(1_900_000_000).unwrap();
        let tps = ThirdPartySecret::new(expected.signature(), NONCE, &SECRET2).unwrap();
        expected
            .add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, tps)
            .unwrap();

        assert_eq!(
            Ok(expected.clone()),
            Macaroon::from_bytes(&expected.to_bytes())
        );
    }

    #[test]
    fn macaroon_from_bytes_rejects_trailing_bytes() {
        let mut bytes = root_macaroon().to_bytes();
        bytes.push(0);

        assert_eq!(
            Err(Error::InvalidEncoding {
                what: "1 trailing bytes".to_owned(),
            }),
            Macaroon::from_bytes(&bytes)
        );
    }

    #[test]
    fn caveat_refs_expose_public_caveat_fields() {
        let mut macaroon = root_macaroon();
        macaroon.add_exact_string("role = admin").unwrap();
        macaroon.add_not_before(1_800_000_000).unwrap();
        macaroon.add_expires(1_900_000_000).unwrap();
        let tps = ThirdPartySecret::new(macaroon.signature(), NONCE, &SECRET2).unwrap();
        macaroon
            .add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, tps)
            .unwrap();

        let caveats: Vec<_> = macaroon.caveats().collect();

        assert_eq!(
            vec![
                CaveatRef::ExactString {
                    what: "role = admin",
                },
                CaveatRef::NotBefore {
                    when: 1_800_000_000,
                },
                CaveatRef::Expires {
                    when: 1_900_000_000,
                },
                CaveatRef::ThirdParty {
                    location: AUTH_LOCATION,
                    identifier: AUTH_IDENTIFIER,
                },
            ],
            caveats
        );
    }

    mod caveats {
        use super::*;

        #[test]
        fn default_is_expired_first_party_caveat() {
            assert_eq!(Caveat::Expires { when: 0 }, Caveat::default());
        }

        #[test]
        fn first_and_third_party_classification() {
            let exact = Caveat::ExactString {
                what: "ip = 127.0.0.1".to_owned(),
            };
            let expires = Caveat::Expires { when: 1693945396 };
            let not_before = Caveat::NotBefore { when: 1693945396 };
            let third_party = Caveat::ThirdParty {
                location: AUTH_LOCATION.to_owned(),
                identifier: AUTH_IDENTIFIER.to_owned(),
                secret: ThirdPartySecret::new(&root_macaroon().signature, NONCE, &SECRET2).unwrap(),
            };

            assert!(exact.is_first_party());
            assert!(!exact.is_third_party());
            assert!(expires.is_first_party());
            assert!(!expires.is_third_party());
            assert!(not_before.is_first_party());
            assert!(!not_before.is_third_party());
            assert!(!third_party.is_first_party());
            assert!(third_party.is_third_party());
        }

        #[test]
        fn exact_string_round_trips_through_prototk() {
            let expected = Caveat::ExactString {
                what: "role = admin".to_owned(),
            };
            let buf = stack_pack(&expected).to_vec();
            let mut unpacker = Unpacker::new(&buf);
            let got: Caveat = unpacker.unpack().unwrap();
            assert_eq!(expected, got);
            assert!(unpacker.is_empty());
        }

        #[test]
        fn expires_round_trips_through_prototk() {
            let expected = Caveat::Expires { when: u64::MAX };
            let buf = stack_pack(&expected).to_vec();
            let mut unpacker = Unpacker::new(&buf);
            let got: Caveat = unpacker.unpack().unwrap();
            assert_eq!(expected, got);
            assert!(unpacker.is_empty());
        }

        #[test]
        fn not_before_round_trips_through_prototk() {
            let expected = Caveat::NotBefore { when: u64::MAX };
            let buf = stack_pack(&expected).to_vec();
            let mut unpacker = Unpacker::new(&buf);
            let got: Caveat = unpacker.unpack().unwrap();
            assert_eq!(expected, got);
            assert!(unpacker.is_empty());
        }

        #[test]
        fn third_party_round_trips_through_prototk() {
            let expected = Caveat::ThirdParty {
                location: AUTH_LOCATION.to_owned(),
                identifier: AUTH_IDENTIFIER.to_owned(),
                secret: ThirdPartySecret::new(&root_macaroon().signature, NONCE, &SECRET2).unwrap(),
            };
            let buf = stack_pack(&expected).to_vec();
            let mut unpacker = Unpacker::new(&buf);
            let got: Caveat = unpacker.unpack().unwrap();
            assert_eq!(expected, got);
            assert!(unpacker.is_empty());
        }

        #[test]
        fn exact_string() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            )
            .unwrap();
            macaroon
                .add_exact_string("ip = 127.0.0.1".to_owned())
                .unwrap();
            assert_eq!(
                Macaroon {
                    location: ROOT_LOCATION.to_owned(),
                    identifier: ROOT_IDENTIFIER.to_owned(),
                    signature: Secret {
                        bytes: [
                            0x6c, 0x85, 0xbc, 0x5f, 0xc9, 0x15, 0x84, 0x31, 0xcf, 0x2e, 0x6f, 0xce,
                            0x5f, 0xc9, 0xa0, 0x7f, 0x85, 0x9e, 0x86, 0x6e, 0x6a, 0xc8, 0xbd, 0xce,
                            0xed, 0x3b, 0x34, 0x2b, 0x34, 0xf2, 0xfd, 0x86,
                        ],
                    },
                    caveats: vec![Caveat::ExactString {
                        what: "ip = 127.0.0.1".to_owned(),
                    }],
                },
                macaroon
            );
            expect(
                macaroon,
                "
location: http://example.org/macaroons
identifier: alice@example.org
signature: [redacted]
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
            )
            .unwrap();
            macaroon.add_expires(1693945396).unwrap();
            assert_eq!(
                Macaroon {
                    location: ROOT_LOCATION.to_owned(),
                    identifier: ROOT_IDENTIFIER.to_owned(),
                    signature: Secret {
                        bytes: [
                            0x46, 0xba, 0x0d, 0x66, 0x77, 0xfd, 0xf7, 0xbe, 0x67, 0x16, 0x7f, 0x27,
                            0x9e, 0xb4, 0x1a, 0x34, 0x71, 0xf7, 0xb8, 0x99, 0xe7, 0xcf, 0x55, 0x42,
                            0xff, 0x9b, 0xbd, 0xd0, 0xbb, 0xed, 0x2f, 0x02,
                        ],
                    },
                    caveats: vec![Caveat::Expires { when: 1693945396 }],
                },
                macaroon
            );
            expect(
                macaroon,
                "
location: http://example.org/macaroons
identifier: alice@example.org
signature: [redacted]
1 caveat
expires: when=1693945396
",
            )
        }

        #[test]
        fn not_before() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            )
            .unwrap();
            macaroon.add_not_before(1693945396).unwrap();
            expect(
                macaroon,
                "
location: http://example.org/macaroons
identifier: alice@example.org
signature: [redacted]
1 caveat
not-before: when=1693945396
",
            )
        }

        #[test]
        fn third_party() {
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            )
            .unwrap();
            let tps = ThirdPartySecret::new(&macaroon.signature, NONCE, &SECRET2).unwrap();
            macaroon
                .add_third_party_caveat(
                    "http://example.net/auth".to_owned(),
                    "bob@example.net".to_owned(),
                    tps,
                )
                .unwrap();
            assert_eq!(
                Macaroon {
                    location: ROOT_LOCATION.to_owned(),
                    identifier: ROOT_IDENTIFIER.to_owned(),
                    signature: Secret {
                        bytes: [
                            0x5b, 0x35, 0x2e, 0x0a, 0x04, 0x49, 0x7d, 0x12, 0x87, 0xc2, 0xdd, 0xd6,
                            0x4d, 0xb6, 0x67, 0x61, 0xa4, 0xe8, 0x67, 0x74, 0xcc, 0x14, 0x5b, 0xcf,
                            0x63, 0x63, 0x9c, 0xb8, 0x13, 0x43, 0x26, 0x6c,
                        ],
                    },
                    caveats: vec![Caveat::ThirdParty {
                        location: AUTH_LOCATION.to_owned(),
                        identifier: AUTH_IDENTIFIER.to_owned(),
                        secret: ThirdPartySecret::new(&root_macaroon().signature, NONCE, &SECRET2,)
                            .unwrap(),
                    }],
                },
                macaroon
            );
            expect(
                macaroon,
                "
location: http://example.org/macaroons
identifier: alice@example.org
signature: [redacted]
1 caveat
third-party: location=http://example.net/auth identifier=bob@example.net
",
            )
        }

        #[test]
        fn third_party_random_generates_a_usable_secret() {
            let mut macaroon = root_macaroon();
            let tps = ThirdPartySecret::random(macaroon.signature(), &SECRET2).unwrap();
            macaroon
                .add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, tps)
                .unwrap();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            let verifier = Verifier::default();
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[discharge]));
        }
    }

    mod covering_set {
        use super::*;

        #[test]
        fn no_third_party_caveats_returns_empty_cover() {
            let root = root_macaroon();
            let candidates = vec![discharge_macaroon()];

            assert_eq!(Ok(Vec::<Macaroon>::new()), root.covering_set(&candidates));
            assert_eq!(
                Ok(Vec::<&Macaroon>::new()),
                root.covering_set_refs(&candidates)
            );
        }

        #[test]
        fn missing_discharge_is_reported_by_public_identity() {
            let root = root_with_third_party_caveat();

            assert_eq!(
                Err(Error::MissingMacaroon {
                    location: AUTH_LOCATION.to_owned(),
                    identifier: AUTH_IDENTIFIER.to_owned(),
                }),
                root.covering_set(&[])
            );
        }

        #[test]
        fn refs_are_returned_from_candidate_slice() {
            let root = root_with_third_party_caveat();
            let noise = Macaroon::new(
                "http://noise.example.invalid",
                "noise@example.invalid",
                SECRET,
            )
            .unwrap();
            let mut discharge = discharge_macaroon();
            root.bind_discharge(&mut discharge).unwrap();
            let candidates = vec![noise, discharge];

            let cover = root.covering_set_refs(&candidates).unwrap();

            assert_eq!(vec![&candidates[1]], cover);
            assert!(std::ptr::eq(&candidates[1], cover[0]));
        }

        #[test]
        fn duplicate_public_identities_select_first_candidate() {
            let root = root_with_third_party_caveat();
            let mut first_candidate =
                Macaroon::new(AUTH_LOCATION, AUTH_IDENTIFIER, SECRET3).unwrap();
            first_candidate
                .add_exact_string("candidate = first")
                .unwrap();
            let mut second_candidate = discharge_macaroon();
            root.bind_discharge(&mut second_candidate).unwrap();
            let candidates = vec![first_candidate.clone(), second_candidate];

            assert_eq!(Ok(vec![first_candidate]), root.covering_set(&candidates));
        }

        #[test]
        fn duplicate_third_party_caveats_share_one_discharge() {
            let mut root = root_macaroon();
            let first_secret = ThirdPartySecret::new(root.signature(), NONCE, &SECRET2).unwrap();
            root.add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, first_secret)
                .unwrap();
            let second_secret = ThirdPartySecret::new(root.signature(), NONCE2, &SECRET2).unwrap();
            root.add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, second_secret)
                .unwrap();
            let mut discharge = discharge_macaroon();
            root.bind_discharge(&mut discharge).unwrap();
            let candidates = vec![discharge.clone()];

            assert_eq!(Ok(vec![discharge]), root.covering_set(&candidates));
        }

        #[test]
        fn nested_discharges_are_selected_from_many_candidates() {
            let mut root = root_macaroon();
            let root_secret = ThirdPartySecret::new(root.signature(), NONCE, &SECRET2).unwrap();
            root.add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, root_secret)
                .unwrap();
            let mut first_discharge = discharge_macaroon();
            let nested_secret =
                ThirdPartySecret::new(first_discharge.signature(), NONCE2, &SECRET3).unwrap();
            first_discharge
                .add_third_party_caveat(NESTED_AUTH_LOCATION, NESTED_AUTH_IDENTIFIER, nested_secret)
                .unwrap();
            let second_discharge =
                Macaroon::new(NESTED_AUTH_LOCATION, NESTED_AUTH_IDENTIFIER, SECRET3).unwrap();

            let mut expected_first = first_discharge;
            let mut expected_second = second_discharge;
            root.bind_discharge(&mut expected_first).unwrap();
            root.bind_discharge(&mut expected_second).unwrap();
            let noise = Macaroon::new(
                "http://noise.example.invalid",
                "noise@example.invalid",
                SECRET,
            )
            .unwrap();
            let candidates = vec![
                noise.clone(),
                expected_second.clone(),
                expected_first.clone(),
                noise,
            ];

            let cover = root.covering_set(&candidates);

            assert_eq!(
                Ok(vec![expected_first.clone(), expected_second.clone()]),
                cover
            );
            assert_eq!(
                Ok(()),
                Verifier::default().verify(&root, &SECRET, &[expected_first, expected_second])
            );
        }

        #[test]
        fn nested_missing_discharge_reports_nested_identity() {
            let mut root = root_macaroon();
            let root_secret = ThirdPartySecret::new(root.signature(), NONCE, &SECRET2).unwrap();
            root.add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, root_secret)
                .unwrap();
            let mut first_discharge = discharge_macaroon();
            let nested_secret =
                ThirdPartySecret::new(first_discharge.signature(), NONCE2, &SECRET3).unwrap();
            first_discharge
                .add_third_party_caveat(NESTED_AUTH_LOCATION, NESTED_AUTH_IDENTIFIER, nested_secret)
                .unwrap();

            assert_eq!(
                Err(Error::MissingMacaroon {
                    location: NESTED_AUTH_LOCATION.to_owned(),
                    identifier: NESTED_AUTH_IDENTIFIER.to_owned(),
                }),
                root.covering_set(&[first_discharge])
            );
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
            )
            .unwrap();
            macaroon
                .add_exact_string("ip = 127.0.0.1".to_owned())
                .unwrap();
            let mut verifier = Verifier::default();
            verifier.add_context("ip = 127.0.0.1".to_owned());
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
        }

        #[test]
        fn fluent_context_and_time_setup() {
            let mut macaroon = root_macaroon();
            macaroon.add_exact_string("ip = 127.0.0.1").unwrap();
            macaroon.add_expires(1693945396).unwrap();
            let verifier = Verifier::new()
                .with_context("ip = 127.0.0.1")
                .with_current_time(1693945395);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
        }

        #[test]
        fn exact_string_rejects_missing_context() {
            let mut macaroon = root_macaroon();
            macaroon
                .add_exact_string("ip = 127.0.0.1".to_owned())
                .unwrap();
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn exact_string_rejects_non_exact_context() {
            let mut macaroon = root_macaroon();
            macaroon
                .add_exact_string("ip = 127.0.0.1".to_owned())
                .unwrap();
            let mut verifier = Verifier::default();
            verifier.add_context("ip = 127.0.0.1 ".to_owned());
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn exact_string_accepts_matching_context_among_others() {
            let mut macaroon = root_macaroon();
            macaroon
                .add_exact_string("ip = 127.0.0.1".to_owned())
                .unwrap();
            let mut verifier = Verifier::default();
            verifier.add_context("role = admin".to_owned());
            verifier.add_context("ip = 127.0.0.1".to_owned());
            verifier.add_context("method = GET".to_owned());
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
        }

        #[test]
        fn caveat_order_is_signed() {
            let mut macaroon = root_macaroon();
            macaroon.add_exact_string("method = GET").unwrap();
            macaroon.add_exact_string("path = /v1/files").unwrap();
            let verifier = Verifier::new()
                .with_context("method = GET")
                .with_context("path = /v1/files");

            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));

            macaroon.caveats.swap(0, 1);

            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn exact_string_rejects_tampered_caveat() {
            let mut macaroon = root_macaroon();
            macaroon
                .add_exact_string("ip = 127.0.0.1".to_owned())
                .unwrap();
            macaroon.caveats[0] = Caveat::ExactString {
                what: "ip = 127.0.0.2".to_owned(),
            };
            let mut verifier = Verifier::default();
            verifier.add_context("ip = 127.0.0.2".to_owned());
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn expires() {
            const NOW: u64 = 1693945396;
            let mut macaroon = Macaroon::new(
                "http://example.org/macaroons".to_owned(),
                "alice@example.org".to_owned(),
                SECRET,
            )
            .unwrap();
            macaroon.add_expires(NOW).unwrap();
            let mut verifier = Verifier::default();
            verifier.set_current_time(NOW);
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
            verifier.set_current_time(NOW - 1);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
            verifier.set_current_time(NOW + 1);
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn expires_accepts_u64_max_until_time_reaches_it() {
            let mut macaroon = root_macaroon();
            macaroon.add_expires(u64::MAX).unwrap();
            let mut verifier = Verifier::default();
            verifier.set_current_time(u64::MAX - 1);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
            verifier.set_current_time(u64::MAX);
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn expires_rejects_when_verifier_time_is_not_set() {
            let mut macaroon = root_macaroon();
            macaroon.add_expires(u64::MAX).unwrap();

            assert_eq!(
                Err(Error::ProofInvalid),
                Verifier::default().verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn not_before() {
            const NOW: u64 = 1693945396;
            let mut macaroon = root_macaroon();
            macaroon.add_not_before(NOW).unwrap();
            let mut verifier = Verifier::default();
            verifier.set_current_time(NOW - 1);
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
            verifier.set_current_time(NOW);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
            verifier.set_current_time(NOW + 1);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
        }

        #[test]
        fn not_before_accepts_zero_for_any_set_time() {
            let mut macaroon = root_macaroon();
            macaroon.add_not_before(0).unwrap();
            let mut verifier = Verifier::default();
            verifier.set_current_time(0);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
            verifier.set_current_time(u64::MAX);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
        }

        #[test]
        fn not_before_rejects_when_verifier_time_is_not_set() {
            let mut macaroon = root_macaroon();
            macaroon.add_not_before(0).unwrap();

            assert_eq!(
                Err(Error::ProofInvalid),
                Verifier::default().verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn not_before_and_expires_define_half_open_window() {
            const START: u64 = 1693945396;
            const END: u64 = 1693945400;
            let mut macaroon = root_macaroon();
            macaroon.add_not_before(START).unwrap();
            macaroon.add_expires(END).unwrap();
            let mut verifier = Verifier::default();
            verifier.set_current_time(START - 1);
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
            verifier.set_current_time(START);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
            verifier.set_current_time(END - 1);
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[]));
            verifier.set_current_time(END);
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
        }

        #[test]
        fn proof_invalid_for_wrong_root_secret() {
            let macaroon = root_macaroon();
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET2, &[])
            );
        }

        #[test]
        fn third_party() {
            let mut macaroon = root_macaroon();
            assert_eq!(
                &[
                    0xdf, 0x8d, 0x9f, 0x90, 0xd5, 0xf1, 0xa6, 0xa3, 0xc6, 0xaa, 0x89, 0x70, 0x1,
                    0xec, 0x26, 0xa6, 0x5d, 0x9, 0x69, 0x21, 0x53, 0xeb, 0x40, 0x85, 0x47, 0xbe,
                    0x5f, 0xb3, 0x8a, 0xc9, 0x8, 0x5b
                ],
                &macaroon.signature.bytes
            );
            let tps = ThirdPartySecret::new(&macaroon.signature, NONCE, &SECRET2).unwrap();
            macaroon
                .add_third_party_caveat(
                    "http://example.net/auth".to_owned(),
                    "bob@example.net".to_owned(),
                    tps,
                )
                .unwrap();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[])
            );
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[discharge]));
        }

        #[test]
        fn third_party_rejects_unbound_discharge() {
            let macaroon = root_with_third_party_caveat();
            let discharge = discharge_macaroon();
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_rejects_wrong_discharge_identifier() {
            let macaroon = root_with_third_party_caveat();
            let mut discharge =
                Macaroon::new(AUTH_LOCATION, NESTED_AUTH_IDENTIFIER, SECRET2).unwrap();
            macaroon.bind_discharge(&mut discharge).unwrap();
            let verifier = Verifier::default();

            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_rejects_wrong_discharge_secret() {
            let macaroon = root_with_third_party_caveat();
            let mut discharge = Macaroon::new(
                AUTH_LOCATION.to_owned(),
                AUTH_IDENTIFIER.to_owned(),
                SECRET3,
            )
            .unwrap();
            macaroon.bind_discharge(&mut discharge).unwrap();
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_rejects_tampered_secret_nonce_tail() {
            let mut macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            if let Caveat::ThirdParty { secret, .. } = &mut macaroon.caveats[0] {
                secret.nonce[NONCE_BYTES] ^= 0xff;
            }
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_rejects_tampered_secret_box_bytes() {
            let mut macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            if let Caveat::ThirdParty { secret, .. } = &mut macaroon.caveats[0] {
                secret.box_bytes[0] ^= 0xff;
            }
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_rejects_tampered_secret_data() {
            let mut macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            if let Caveat::ThirdParty { secret, .. } = &mut macaroon.caveats[0] {
                secret.data[0] ^= 0xff;
            }
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_bound_to_one_root_rejects_other_root() {
            let mut first_root = root_macaroon();
            let first_secret =
                ThirdPartySecret::new(first_root.signature(), NONCE, &SECRET2).unwrap();
            first_root
                .add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, first_secret)
                .unwrap();

            let mut second_root = root_macaroon();
            second_root.add_exact_string("method = GET").unwrap();
            let second_secret =
                ThirdPartySecret::new(second_root.signature(), NONCE2, &SECRET2).unwrap();
            second_root
                .add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, second_secret)
                .unwrap();

            let mut discharge = discharge_macaroon();
            first_root.bind_discharge(&mut discharge).unwrap();
            let verifier = Verifier::new().with_context("method = GET");

            assert_eq!(
                Ok(()),
                verifier.verify(&first_root, &SECRET, &[discharge.clone()])
            );
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&second_root, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_caveat_location_hint_is_not_signed() {
            let mut macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            if let Caveat::ThirdParty { location, .. } = &mut macaroon.caveats[0] {
                *location = "http://mirror.example.net/auth".to_owned();
            }

            assert_eq!(
                Ok(()),
                Verifier::default().verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_discharge_location_hint_is_not_signed() {
            let macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            discharge.location = "http://mirror.example.net/auth".to_owned();

            assert_eq!(
                Ok(()),
                Verifier::default().verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_ignores_unrelated_discharges() {
            let macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            macaroon.bind_discharge(&mut discharge).unwrap();
            let mut unrelated =
                Macaroon::new(NESTED_AUTH_LOCATION, NESTED_AUTH_IDENTIFIER, SECRET3).unwrap();
            unrelated.add_exact_string("group = admin").unwrap();

            assert_eq!(
                Ok(()),
                Verifier::default().verify(&macaroon, &SECRET, &[unrelated, discharge])
            );
        }

        #[test]
        fn third_party_accepts_multiple_discharges_in_any_order() {
            let mut macaroon = root_macaroon();
            let first_secret =
                ThirdPartySecret::new(macaroon.signature(), NONCE, &SECRET2).unwrap();
            macaroon
                .add_third_party_caveat(AUTH_LOCATION, AUTH_IDENTIFIER, first_secret)
                .unwrap();
            let second_secret =
                ThirdPartySecret::new(macaroon.signature(), NONCE2, &SECRET3).unwrap();
            macaroon
                .add_third_party_caveat(NESTED_AUTH_LOCATION, NESTED_AUTH_IDENTIFIER, second_secret)
                .unwrap();
            let mut first_discharge = discharge_macaroon();
            let mut second_discharge =
                Macaroon::new(NESTED_AUTH_LOCATION, NESTED_AUTH_IDENTIFIER, SECRET3).unwrap();
            macaroon.bind_discharge(&mut first_discharge).unwrap();
            macaroon.bind_discharge(&mut second_discharge).unwrap();
            let verifier = Verifier::default();

            assert_eq!(
                Ok(()),
                verifier.verify(
                    &macaroon,
                    &SECRET,
                    &[first_discharge.clone(), second_discharge.clone()]
                )
            );
            assert_eq!(
                Ok(()),
                verifier.verify(&macaroon, &SECRET, &[second_discharge, first_discharge])
            );
        }

        #[test]
        fn third_party_accepts_discharge_with_satisfied_first_party_caveat() {
            let macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            discharge
                .add_exact_string("group = admin".to_owned())
                .unwrap();
            macaroon.bind_discharge(&mut discharge).unwrap();
            let mut verifier = Verifier::default();
            verifier.add_context("group = admin".to_owned());
            assert_eq!(Ok(()), verifier.verify(&macaroon, &SECRET, &[discharge]));
        }

        #[test]
        fn third_party_rejects_tampered_discharge_caveat() {
            let macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            discharge.add_exact_string("group = admin").unwrap();
            macaroon.bind_discharge(&mut discharge).unwrap();
            discharge.caveats[0] = Caveat::ExactString {
                what: "group = owner".to_owned(),
            };
            let verifier = Verifier::new().with_context("group = owner");

            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }

        #[test]
        fn third_party_rejects_discharge_with_unsatisfied_first_party_caveat() {
            let macaroon = root_with_third_party_caveat();
            let mut discharge = discharge_macaroon();
            discharge
                .add_exact_string("group = admin".to_owned())
                .unwrap();
            macaroon.bind_discharge(&mut discharge).unwrap();
            let verifier = Verifier::default();
            assert_eq!(
                Err(Error::ProofInvalid),
                verifier.verify(&macaroon, &SECRET, &[discharge])
            );
        }
    }

    mod request_builder {
        use super::*;

        #[derive(Clone, Debug)]
        struct StaticLoader {
            location: &'static str,
            macaroons: Vec<Macaroon>,
        }

        impl StaticLoader {
            fn new(location: &'static str, macaroons: Vec<Macaroon>) -> Self {
                Self {
                    location,
                    macaroons,
                }
            }
        }

        impl Loader for StaticLoader {
            fn location(&self) -> &'static str {
                self.location
            }

            fn lookup(&self, identifier: &str) -> Result<Macaroon, Error> {
                for macaroon in &self.macaroons {
                    if crypto::str_eq(macaroon.identifier(), identifier) {
                        return Ok(macaroon.clone());
                    }
                }
                Err(Error::MissingLoader {
                    what: format!("{}/{}", self.location, identifier),
                })
            }
        }

        fn request_builder_with_root(root: Macaroon) -> RequestBuilder {
            let mut builder = RequestBuilder::new();
            builder
                .add_loader(StaticLoader::new(ROOT_LOCATION, vec![root]))
                .unwrap();
            builder
        }

        #[test]
        fn missing_root_loader() {
            let builder = RequestBuilder::new();
            assert_eq!(
                Err(Error::MissingLoader {
                    what: ROOT_LOCATION.to_owned(),
                }),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn rejects_loader_location_mismatch() {
            let wrong_location = Macaroon::new(
                "http://wrong.example.org/macaroons".to_owned(),
                ROOT_IDENTIFIER.to_owned(),
                SECRET,
            )
            .unwrap();
            let builder = request_builder_with_root(wrong_location);
            assert_eq!(
                Err(Error::LocationMismatch {
                    expected: ROOT_LOCATION.to_owned(),
                    actual: "http://wrong.example.org/macaroons".to_owned(),
                }),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn request_without_third_party_caveats_returns_root_only() {
            let root = root_macaroon();
            let builder = request_builder_with_root(root.clone());
            assert_eq!(
                Ok((root, Vec::new())),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn fluent_loader_setup() {
            let root = root_macaroon();
            let builder = RequestBuilder::new()
                .with_loader(StaticLoader::new(ROOT_LOCATION, vec![root.clone()]))
                .unwrap();
            assert_eq!(
                Ok((root, Vec::new())),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn duplicate_loader_registration_is_rejected() {
            let mut builder = RequestBuilder::new();
            assert_eq!(
                Ok(()),
                builder.add_loader(StaticLoader::new(ROOT_LOCATION, vec![root_macaroon()]))
            );
            assert_eq!(
                Err(Error::DuplicateLoader {
                    location: ROOT_LOCATION.to_owned(),
                }),
                builder.add_loader(StaticLoader::new(ROOT_LOCATION, Vec::new()))
            );
        }

        #[test]
        fn single_third_party_missing_loader_is_reported() {
            let root = root_with_third_party_caveat();
            let builder = request_builder_with_root(root);

            assert_eq!(
                Err(Error::MissingLoader {
                    what: AUTH_LOCATION.to_owned(),
                }),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn third_party_location_hint_tampering_changes_request_assembly() {
            let mut root = root_with_third_party_caveat();
            if let Caveat::ThirdParty { location, .. } = &mut root.caveats[0] {
                *location = NESTED_AUTH_LOCATION.to_owned();
            }
            let discharge = Macaroon::new(NESTED_AUTH_LOCATION, AUTH_IDENTIFIER, SECRET2).unwrap();

            let mut builder = request_builder_with_root(root.clone());
            builder
                .add_loader(StaticLoader::new(AUTH_LOCATION, vec![discharge.clone()]))
                .unwrap();
            assert_eq!(
                Err(Error::MissingLoader {
                    what: NESTED_AUTH_LOCATION.to_owned(),
                }),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );

            let mut builder = request_builder_with_root(root.clone());
            builder
                .add_loader(StaticLoader::new(
                    NESTED_AUTH_LOCATION,
                    vec![discharge.clone()],
                ))
                .unwrap();
            let mut expected_discharge = discharge;
            root.bind_discharge(&mut expected_discharge).unwrap();

            assert_eq!(
                Ok((root.clone(), vec![expected_discharge.clone()])),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
            assert_eq!(
                Ok(()),
                Verifier::default().verify(&root, &SECRET, &[expected_discharge])
            );
        }

        #[test]
        fn discharge_lookup_error_is_returned() {
            let root = root_with_third_party_caveat();
            let mut builder = request_builder_with_root(root);
            builder
                .add_loader(StaticLoader::new(AUTH_LOCATION, Vec::new()))
                .unwrap();

            assert_eq!(
                Err(Error::MissingLoader {
                    what: format!("{AUTH_LOCATION}/{AUTH_IDENTIFIER}"),
                }),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn rejects_discharge_loader_location_mismatch() {
            let root = root_with_third_party_caveat();
            let wrong_location = "http://wrong.example.net/auth";
            let wrong_discharge = Macaroon::new(wrong_location, AUTH_IDENTIFIER, SECRET2).unwrap();
            let mut builder = request_builder_with_root(root);
            builder
                .add_loader(StaticLoader::new(AUTH_LOCATION, vec![wrong_discharge]))
                .unwrap();

            assert_eq!(
                Err(Error::LocationMismatch {
                    expected: AUTH_LOCATION.to_owned(),
                    actual: wrong_location.to_owned(),
                }),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn single_third_party_caveat_returns_bound_discharge() {
            let root = root_with_third_party_caveat();
            let discharge = discharge_macaroon();
            let mut builder = request_builder_with_root(root.clone());
            builder
                .add_loader(StaticLoader::new(AUTH_LOCATION, vec![discharge.clone()]))
                .unwrap();
            let mut expected_discharge = discharge;
            root.bind_discharge(&mut expected_discharge).unwrap();
            assert_eq!(
                Ok((root.clone(), vec![expected_discharge.clone()])),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
            let verifier = Verifier::default();
            assert_eq!(
                Ok(()),
                verifier.verify(&root, &SECRET, &[expected_discharge])
            );
        }

        #[test]
        fn nested_third_party_caveats_are_loaded_and_bound() {
            let mut root = root_macaroon();
            let root_secret = ThirdPartySecret::new(root.signature(), NONCE, &SECRET2).unwrap();
            root.add_third_party_caveat(
                AUTH_LOCATION.to_owned(),
                AUTH_IDENTIFIER.to_owned(),
                root_secret,
            )
            .unwrap();
            let mut first_discharge = discharge_macaroon();
            let nested_secret =
                ThirdPartySecret::new(first_discharge.signature(), NONCE2, &SECRET3).unwrap();
            first_discharge
                .add_third_party_caveat(
                    NESTED_AUTH_LOCATION.to_owned(),
                    NESTED_AUTH_IDENTIFIER.to_owned(),
                    nested_secret,
                )
                .unwrap();
            let second_discharge = Macaroon::new(
                NESTED_AUTH_LOCATION.to_owned(),
                NESTED_AUTH_IDENTIFIER.to_owned(),
                SECRET3,
            )
            .unwrap();

            let mut builder = request_builder_with_root(root.clone());
            builder
                .add_loader(StaticLoader::new(
                    AUTH_LOCATION,
                    vec![first_discharge.clone()],
                ))
                .unwrap();
            builder
                .add_loader(StaticLoader::new(
                    NESTED_AUTH_LOCATION,
                    vec![second_discharge.clone()],
                ))
                .unwrap();

            let mut expected_first = first_discharge;
            let mut expected_second = second_discharge;
            root.bind_discharge(&mut expected_first).unwrap();
            root.bind_discharge(&mut expected_second).unwrap();
            let expected_discharges = vec![expected_first, expected_second];

            assert_eq!(
                Ok((root.clone(), expected_discharges.clone())),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
            let verifier = Verifier::default();
            assert_eq!(
                Ok(()),
                verifier.verify(&root, &SECRET, &expected_discharges)
            );
        }

        #[test]
        fn nested_third_party_missing_loader_is_reported() {
            let mut root = root_macaroon();
            let root_secret = ThirdPartySecret::new(root.signature(), NONCE, &SECRET2).unwrap();
            root.add_third_party_caveat(
                AUTH_LOCATION.to_owned(),
                AUTH_IDENTIFIER.to_owned(),
                root_secret,
            )
            .unwrap();
            let mut first_discharge = discharge_macaroon();
            let nested_secret =
                ThirdPartySecret::new(first_discharge.signature(), NONCE2, &SECRET3).unwrap();
            first_discharge
                .add_third_party_caveat(
                    NESTED_AUTH_LOCATION.to_owned(),
                    NESTED_AUTH_IDENTIFIER.to_owned(),
                    nested_secret,
                )
                .unwrap();

            let mut builder = request_builder_with_root(root);
            builder
                .add_loader(StaticLoader::new(AUTH_LOCATION, vec![first_discharge]))
                .unwrap();

            assert_eq!(
                Err(Error::MissingLoader {
                    what: NESTED_AUTH_LOCATION.to_owned(),
                }),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
        }

        #[test]
        fn duplicate_third_party_caveats_share_one_bound_discharge() {
            let mut root = root_macaroon();
            let first_secret = ThirdPartySecret::new(root.signature(), NONCE, &SECRET2).unwrap();
            root.add_third_party_caveat(
                AUTH_LOCATION.to_owned(),
                AUTH_IDENTIFIER.to_owned(),
                first_secret,
            )
            .unwrap();
            let second_secret = ThirdPartySecret::new(root.signature(), NONCE2, &SECRET2).unwrap();
            root.add_third_party_caveat(
                AUTH_LOCATION.to_owned(),
                AUTH_IDENTIFIER.to_owned(),
                second_secret,
            )
            .unwrap();
            let discharge = discharge_macaroon();
            let mut builder = request_builder_with_root(root.clone());
            builder
                .add_loader(StaticLoader::new(AUTH_LOCATION, vec![discharge.clone()]))
                .unwrap();

            let mut expected_discharge = discharge;
            root.bind_discharge(&mut expected_discharge).unwrap();
            let expected_discharges = vec![expected_discharge];

            assert_eq!(
                Ok((root.clone(), expected_discharges.clone())),
                builder.prepare_request(ROOT_LOCATION, ROOT_IDENTIFIER)
            );
            let verifier = Verifier::default();
            assert_eq!(
                Ok(()),
                verifier.verify(&root, &SECRET, &expected_discharges)
            );
        }
    }

    mod generated_proofs {
        use super::*;
        use proptest::prelude::*;

        const MAX_GENERATED_CAVEATS: usize = 4;
        const MAX_GENERATED_DEPTH: usize = 3;
        const MAX_GENERATED_MACAROONS: usize = 64;
        const PROPERTY_NOW: u64 = 1_000_000;

        #[derive(Clone)]
        struct GeneratedProof {
            root: GeneratedNode,
            node_count: usize,
            max_depth: usize,
            max_caveats: usize,
        }

        #[derive(Clone)]
        struct GeneratedNode {
            location: String,
            identifier: String,
            secret: Secret,
            caveats: Vec<GeneratedCaveat>,
        }

        #[derive(Clone)]
        enum GeneratedCaveat {
            ExactString {
                what: String,
            },
            Expires {
                when: u64,
            },
            NotBefore {
                when: u64,
            },
            ThirdParty {
                nonce: Nonce,
                child: Box<GeneratedNode>,
            },
        }

        #[derive(Clone)]
        struct BuiltGeneratedProof {
            root: Macaroon,
            root_secret: Secret,
            discharges: Vec<Macaroon>,
        }

        #[derive(Clone, Copy, Debug)]
        enum RejectScenario {
            WrongRootSecret,
            MissingContext,
            Expired,
            NotYetValid,
            MissingDischarge,
            TamperedRootSignature,
            TamperedDischargeSignature,
        }

        struct Entropy {
            bytes: Vec<u8>,
            index: usize,
        }

        impl Entropy {
            fn new(mut bytes: Vec<u8>) -> Self {
                if bytes.is_empty() {
                    bytes.push(0);
                }
                Self { bytes, index: 0 }
            }

            fn next(&mut self) -> u8 {
                let byte = self.bytes[self.index % self.bytes.len()];
                self.index += 1;
                byte
            }

            fn choice(&mut self, upper: usize) -> usize {
                if upper == 0 {
                    0
                } else {
                    usize::from(self.next()) % upper
                }
            }

            fn secret(&mut self, label: u8, id: usize) -> Secret {
                let mut bytes = [0u8; SIGNATURE_BYTES];
                for (offset, byte) in bytes.iter_mut().enumerate() {
                    *byte = self
                        .next()
                        .wrapping_add(label)
                        .wrapping_add((id as u8).wrapping_mul(31))
                        .wrapping_add(offset as u8);
                }
                Secret { bytes }
            }

            fn nonce(&mut self, label: u8, id: usize) -> Nonce {
                let secret = self.secret(label, id);
                let mut bytes = [0u8; NONCE_BYTES];
                bytes.copy_from_slice(&secret.bytes[0..NONCE_BYTES]);
                Nonce::from_bytes(bytes)
            }
        }

        struct GeneratedState {
            entropy: Entropy,
            node_count: usize,
            max_depth: usize,
            max_caveats: usize,
        }

        impl GeneratedState {
            fn new(bytes: Vec<u8>) -> Self {
                Self {
                    entropy: Entropy::new(bytes),
                    node_count: 0,
                    max_depth: 0,
                    max_caveats: 0,
                }
            }

            fn node(&mut self, depth: usize) -> GeneratedNode {
                let id = self.node_count;
                self.node_count += 1;
                self.max_depth = self.max_depth.max(depth);

                let location = format!("generated-location-{id}");
                let identifier = format!("generated-identifier-{id}");
                let secret = self.entropy.secret(0xa5, id);
                let caveat_count = self.entropy.choice(MAX_GENERATED_CAVEATS + 1);
                self.max_caveats = self.max_caveats.max(caveat_count);

                let mut caveats = Vec::with_capacity(caveat_count);
                for caveat_idx in 0..caveat_count {
                    let can_add_child =
                        depth < MAX_GENERATED_DEPTH && self.node_count < MAX_GENERATED_MACAROONS;
                    match self.entropy.choice(if can_add_child { 4 } else { 3 }) {
                        0 => caveats.push(GeneratedCaveat::ExactString {
                            what: format!(
                                "generated-context-{id}-{caveat_idx}-{}",
                                self.entropy.next()
                            ),
                        }),
                        1 => caveats.push(GeneratedCaveat::Expires {
                            when: PROPERTY_NOW
                                + 1
                                + self.entropy.choice(256) as u64
                                + id as u64
                                + caveat_idx as u64,
                        }),
                        2 => caveats.push(GeneratedCaveat::NotBefore {
                            when: PROPERTY_NOW
                                - self.entropy.choice(256) as u64
                                - id as u64
                                - caveat_idx as u64,
                        }),
                        _ => {
                            let nonce = self
                                .entropy
                                .nonce(0x5a, id * MAX_GENERATED_CAVEATS + caveat_idx);
                            let child = Box::new(self.node(depth + 1));
                            caveats.push(GeneratedCaveat::ThirdParty { nonce, child });
                        }
                    }
                }

                GeneratedNode {
                    location,
                    identifier,
                    secret,
                    caveats,
                }
            }
        }

        impl GeneratedProof {
            fn from_entropy(bytes: Vec<u8>) -> Self {
                let mut state = GeneratedState::new(bytes);
                let root = state.node(0);
                Self {
                    root,
                    node_count: state.node_count,
                    max_depth: state.max_depth,
                    max_caveats: state.max_caveats,
                }
            }

            fn build(&self) -> BuiltGeneratedProof {
                let (root, mut discharges) = self.root.build();
                for discharge in &mut discharges {
                    root.bind_discharge(discharge).unwrap();
                }
                BuiltGeneratedProof {
                    root,
                    root_secret: self.root.secret.clone(),
                    discharges,
                }
            }

            fn verifier(&self, now: u64, skip_context: Option<&str>) -> Verifier {
                let mut verifier = Verifier::default();
                verifier.set_current_time(now);
                self.root.add_contexts(&mut verifier, skip_context);
                verifier
            }

            fn verify(
                &self,
                root: &Macaroon,
                root_secret: &Secret,
                discharges: &[Macaroon],
                now: u64,
                skip_context: Option<&str>,
            ) -> Result<(), Error> {
                self.verifier(now, skip_context)
                    .verify(root, root_secret, discharges)
            }

            fn verify_built(&self, built: &BuiltGeneratedProof) -> Result<(), Error> {
                self.verify(
                    &built.root,
                    &built.root_secret,
                    &built.discharges,
                    PROPERTY_NOW,
                    None,
                )
            }

            fn reject_result(
                &self,
                built: &BuiltGeneratedProof,
                scenario: RejectScenario,
            ) -> Result<(), Error> {
                match scenario {
                    RejectScenario::WrongRootSecret => self.verify(
                        &built.root,
                        &different_secret(&built.root_secret),
                        &built.discharges,
                        PROPERTY_NOW,
                        None,
                    ),
                    RejectScenario::MissingContext => {
                        if let Some(context) = self.root.first_context() {
                            self.verify(
                                &built.root,
                                &built.root_secret,
                                &built.discharges,
                                PROPERTY_NOW,
                                Some(&context),
                            )
                        } else {
                            self.tampered_root_signature_result(built)
                        }
                    }
                    RejectScenario::Expired => {
                        if let Some(when) = self.root.first_expiration() {
                            self.verify(
                                &built.root,
                                &built.root_secret,
                                &built.discharges,
                                when,
                                None,
                            )
                        } else {
                            self.tampered_root_signature_result(built)
                        }
                    }
                    RejectScenario::NotYetValid => {
                        if let Some(when) = self.root.first_not_before() {
                            if when > 0 {
                                self.verify(
                                    &built.root,
                                    &built.root_secret,
                                    &built.discharges,
                                    when - 1,
                                    None,
                                )
                            } else {
                                self.tampered_root_signature_result(built)
                            }
                        } else {
                            self.tampered_root_signature_result(built)
                        }
                    }
                    RejectScenario::MissingDischarge => {
                        if built.discharges.is_empty() {
                            self.tampered_root_signature_result(built)
                        } else {
                            let mut discharges = built.discharges.clone();
                            discharges.remove(0);
                            self.verify(
                                &built.root,
                                &built.root_secret,
                                &discharges,
                                PROPERTY_NOW,
                                None,
                            )
                        }
                    }
                    RejectScenario::TamperedRootSignature => {
                        self.tampered_root_signature_result(built)
                    }
                    RejectScenario::TamperedDischargeSignature => {
                        if built.discharges.is_empty() {
                            self.tampered_root_signature_result(built)
                        } else {
                            let mut discharges = built.discharges.clone();
                            tamper_signature(&mut discharges[0].signature);
                            self.verify(
                                &built.root,
                                &built.root_secret,
                                &discharges,
                                PROPERTY_NOW,
                                None,
                            )
                        }
                    }
                }
            }

            fn tampered_root_signature_result(
                &self,
                built: &BuiltGeneratedProof,
            ) -> Result<(), Error> {
                let mut root = built.root.clone();
                tamper_signature(&mut root.signature);
                self.verify(
                    &root,
                    &built.root_secret,
                    &built.discharges,
                    PROPERTY_NOW,
                    None,
                )
            }
        }

        impl GeneratedNode {
            fn build(&self) -> (Macaroon, Vec<Macaroon>) {
                let mut macaroon = Macaroon::new(
                    self.location.clone(),
                    self.identifier.clone(),
                    self.secret.clone(),
                )
                .unwrap();
                let mut discharges = Vec::new();
                for caveat in &self.caveats {
                    match caveat {
                        GeneratedCaveat::ExactString { what } => {
                            macaroon.add_exact_string(what.clone()).unwrap();
                        }
                        GeneratedCaveat::Expires { when } => {
                            macaroon.add_expires(*when).unwrap();
                        }
                        GeneratedCaveat::NotBefore { when } => {
                            macaroon.add_not_before(*when).unwrap();
                        }
                        GeneratedCaveat::ThirdParty { nonce, child } => {
                            let secret =
                                ThirdPartySecret::new(macaroon.signature(), *nonce, &child.secret)
                                    .unwrap();
                            macaroon
                                .add_third_party_caveat(
                                    child.location.clone(),
                                    child.identifier.clone(),
                                    secret,
                                )
                                .unwrap();
                            let (child_macaroon, child_discharges) = child.build();
                            discharges.push(child_macaroon);
                            discharges.extend(child_discharges);
                        }
                    }
                }
                (macaroon, discharges)
            }

            fn add_contexts(&self, verifier: &mut Verifier, skip_context: Option<&str>) {
                for caveat in &self.caveats {
                    match caveat {
                        GeneratedCaveat::ExactString { what } => {
                            if skip_context
                                .map(|skip_context| skip_context != what.as_str())
                                .unwrap_or(true)
                            {
                                verifier.add_context(what.clone());
                            }
                        }
                        GeneratedCaveat::Expires { .. } | GeneratedCaveat::NotBefore { .. } => {}
                        GeneratedCaveat::ThirdParty { child, .. } => {
                            child.add_contexts(verifier, skip_context);
                        }
                    }
                }
            }

            fn first_context(&self) -> Option<String> {
                for caveat in &self.caveats {
                    match caveat {
                        GeneratedCaveat::ExactString { what } => {
                            return Some(what.clone());
                        }
                        GeneratedCaveat::Expires { .. } | GeneratedCaveat::NotBefore { .. } => {}
                        GeneratedCaveat::ThirdParty { child, .. } => {
                            if let Some(context) = child.first_context() {
                                return Some(context);
                            }
                        }
                    }
                }
                None
            }

            fn first_expiration(&self) -> Option<u64> {
                for caveat in &self.caveats {
                    match caveat {
                        GeneratedCaveat::ExactString { .. } => {}
                        GeneratedCaveat::NotBefore { .. } => {}
                        GeneratedCaveat::Expires { when } => {
                            return Some(*when);
                        }
                        GeneratedCaveat::ThirdParty { child, .. } => {
                            if let Some(when) = child.first_expiration() {
                                return Some(when);
                            }
                        }
                    }
                }
                None
            }

            fn first_not_before(&self) -> Option<u64> {
                for caveat in &self.caveats {
                    match caveat {
                        GeneratedCaveat::ExactString { .. } => {}
                        GeneratedCaveat::Expires { .. } => {}
                        GeneratedCaveat::NotBefore { when } => {
                            return Some(*when);
                        }
                        GeneratedCaveat::ThirdParty { child, .. } => {
                            if let Some(when) = child.first_not_before() {
                                return Some(when);
                            }
                        }
                    }
                }
                None
            }
        }

        fn different_secret(secret: &Secret) -> Secret {
            let mut different = secret.clone();
            tamper_signature(&mut different);
            different
        }

        fn tamper_signature(secret: &mut Secret) {
            secret.bytes[0] ^= 0x80;
        }

        fn noise_macaroon(index: usize) -> Macaroon {
            let mut bytes = [0u8; SIGNATURE_BYTES];
            for (offset, byte) in bytes.iter_mut().enumerate() {
                *byte = 0xc3u8
                    .wrapping_add(index as u8)
                    .wrapping_add((offset as u8).wrapping_mul(17));
            }
            let mut macaroon = Macaroon::new(
                format!("noise-location-{index}"),
                format!("noise-identifier-{index}"),
                Secret::from_bytes(bytes),
            )
            .unwrap();
            macaroon
                .add_exact_string(format!("noise-context-{index}"))
                .unwrap();
            macaroon
        }

        fn candidates_with_noise(discharges: &[Macaroon]) -> Vec<Macaroon> {
            let mut candidates = Vec::with_capacity(discharges.len() * 2 + 1);
            for (idx, discharge) in discharges.iter().enumerate() {
                candidates.push(noise_macaroon(idx));
                candidates.push(discharge.clone());
            }
            candidates.push(noise_macaroon(discharges.len()));
            candidates
        }

        fn public_identities(macaroons: &[Macaroon]) -> Vec<(String, String)> {
            let mut identities: Vec<_> = macaroons
                .iter()
                .map(|macaroon| {
                    (
                        macaroon.location().to_owned(),
                        macaroon.identifier().to_owned(),
                    )
                })
                .collect();
            identities.sort();
            identities
        }

        fn reject_scenario() -> impl Strategy<Value = RejectScenario> {
            prop_oneof![
                Just(RejectScenario::WrongRootSecret),
                Just(RejectScenario::MissingContext),
                Just(RejectScenario::Expired),
                Just(RejectScenario::NotYetValid),
                Just(RejectScenario::MissingDischarge),
                Just(RejectScenario::TamperedRootSignature),
                Just(RejectScenario::TamperedDischargeSignature),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig {
                cases: 128,
                max_shrink_iters: 4096,
                .. ProptestConfig::default()
            })]

            #[test]
            fn generated_proof_trees_accept_and_reject(
                entropy in prop::collection::vec(any::<u8>(), 1..=512),
                scenario in reject_scenario(),
            ) {
                let proof = GeneratedProof::from_entropy(entropy);
                let built = proof.build();

                prop_assert!(proof.node_count <= MAX_GENERATED_MACAROONS);
                prop_assert!(proof.max_depth <= MAX_GENERATED_DEPTH);
                prop_assert!(proof.max_caveats <= MAX_GENERATED_CAVEATS);
                let total_macaroons = built.discharges.len() + 1;
                prop_assert_eq!(proof.node_count, total_macaroons);
                prop_assert!(total_macaroons <= MAX_GENERATED_MACAROONS);

                prop_assert_eq!(
                    Ok(()),
                    proof.verify_built(&built)
                );

                prop_assert_eq!(
                    Err(Error::ProofInvalid),
                    proof.reject_result(&built, scenario)
                );
            }

            #[test]
            fn generated_covering_sets_select_required_public_discharges(
                entropy in prop::collection::vec(any::<u8>(), 1..=512),
            ) {
                let proof = GeneratedProof::from_entropy(entropy);
                let built = proof.build();
                let candidates = candidates_with_noise(&built.discharges);
                let cover = built
                    .root
                    .covering_set(&candidates)
                    .expect("generated candidates include every required discharge");

                prop_assert_eq!(
                    public_identities(&built.discharges),
                    public_identities(&cover)
                );
                prop_assert_eq!(
                    Ok(()),
                    proof.verify(
                        &built.root,
                        &built.root_secret,
                        &cover,
                        PROPERTY_NOW,
                        None,
                    )
                );
            }

            #[test]
            fn generated_proofs_verify_independent_of_discharge_order(
                entropy in prop::collection::vec(any::<u8>(), 1..=512),
            ) {
                let proof = GeneratedProof::from_entropy(entropy);
                let mut built = proof.build();
                built.discharges.reverse();

                prop_assert_eq!(
                    Ok(()),
                    proof.verify_built(&built)
                );
            }

            #[test]
            fn arbitrary_bytes_decode_only_as_canonical_macaroons(
                bytes in prop::collection::vec(any::<u8>(), 0..=512),
            ) {
                if let Ok(macaroon) = Macaroon::from_bytes(&bytes) {
                    prop_assert_eq!(bytes, macaroon.to_bytes());
                    prop_assert_eq!(
                        Ok(macaroon.clone()),
                        Macaroon::from_bytes(&macaroon.to_bytes())
                    );
                }
            }
        }
    }
}
