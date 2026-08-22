//! The object-store abstraction.
//!
//! Per the design (§3, §11), **conditional PUT is the only synchronization primitive**, and it is
//! used at exactly two points:
//!
//! * claim an empty offset with `PUT fragment_N` and `If-None-Match: *`
//!   ([`ConditionalStore::put_if_none_match`]); and
//! * advance a known version with `PUT <sst-slot>` and `If-Match: <etag>`
//!   ([`ConditionalStore::put_if_match`]).
//!
//! Everything else in sstdb sits on this trait.  [`ObjectStoreConditional`] backs it with any
//! [`object_store::ObjectStore`] — `InMemory` for tests, `AmazonS3` for production — both of which
//! support the standard HTTP precondition headers this design relies upon.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
#[cfg(feature = "s3")]
use object_store::aws::AmazonS3Builder;
use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::{
    Error as OsError, ObjectStore, ObjectStoreExt, PutMode, PutOptions, UpdateVersion,
};

use crate::{Result, corruption, store_error};

/// An object fetched from the store: its bytes and its ETag.
#[derive(Clone, Debug)]
pub struct Object {
    /// The object's bytes.
    pub bytes: Bytes,
    /// The object's ETag, used as the read-version for conditional updates.
    pub etag: String,
}

/// The outcome of a conditional PUT.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriteOutcome {
    /// The PUT succeeded; carries the new ETag of the object.
    Written(String),
    /// The conditional PUT's precondition failed: the slot was already claimed
    /// (`If-None-Match`) or the version moved underneath us (`If-Match`).
    PreconditionFailed,
}

/// The thin conditional object-store trait sstdb is built on.
#[async_trait]
pub trait ConditionalStore: Send + Sync {
    /// Fetch the object at `path`, or `None` if it does not exist.
    async fn get(&self, path: &str) -> Result<Option<Object>>;

    /// Fetch just the ETag of the object at `path` (a HEAD), or `None` if it does not exist.
    async fn head(&self, path: &str) -> Result<Option<String>>;

    /// `PUT path` with `If-None-Match: *`: claim an empty slot.  Returns
    /// [`WriteOutcome::PreconditionFailed`] if the object already exists.
    async fn put_if_none_match(&self, path: &str, bytes: Vec<u8>) -> Result<WriteOutcome>;

    /// `PUT path` with `If-Match: <etag>`: advance a known version.  Returns
    /// [`WriteOutcome::PreconditionFailed`] if the current ETag no longer matches.
    async fn put_if_match(&self, path: &str, bytes: Vec<u8>, etag: &str) -> Result<WriteOutcome>;
}

/// A [`ConditionalStore`] backed by any [`object_store::ObjectStore`].
#[derive(Clone, Debug)]
pub struct ObjectStoreConditional {
    inner: Arc<dyn ObjectStore>,
}

impl ObjectStoreConditional {
    /// Wrap an existing object store.
    pub fn new(inner: Arc<dyn ObjectStore>) -> Self {
        Self { inner }
    }

    /// An in-memory store, suitable for tests.  It supports the conditional puts this design
    /// requires, so it exercises the same code paths as S3.
    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemory::new()))
    }

    /// Back the store with S3 (or an S3-compatible service), configured from the standard
    /// `AWS_*` environment variables, for the given bucket.  Conditional PUT (`ETagMatch`) is the
    /// default for the AWS backend, which is exactly what this design requires.
    ///
    /// Available with the `s3` cargo feature.
    #[cfg(feature = "s3")]
    pub fn s3_from_env(bucket: &str) -> Result<Self> {
        let s3 = AmazonS3Builder::from_env()
            .with_bucket_name(bucket)
            .build()
            .map_err(store_error)?;
        Ok(Self::new(Arc::new(s3)))
    }
}

/// Extract the ETag from an object's metadata, synthesizing a deterministic placeholder when the
/// backend does not surface one (some stores omit it for HEAD on tiny objects).
fn require_etag(e_tag: Option<String>) -> Result<String> {
    e_tag.ok_or_else(|| corruption("object store returned no ETag"))
}

#[async_trait]
impl ConditionalStore for ObjectStoreConditional {
    async fn get(&self, path: &str) -> Result<Option<Object>> {
        let location = ObjectPath::from(path);
        let result = match self.inner.get(&location).await {
            Ok(r) => r,
            Err(OsError::NotFound { .. }) => return Ok(None),
            Err(e) => return Err(store_error(e)),
        };
        let etag = require_etag(result.meta.e_tag.clone())?;
        let bytes = result.bytes().await.map_err(store_error)?;
        Ok(Some(Object { bytes, etag }))
    }

    async fn head(&self, path: &str) -> Result<Option<String>> {
        let location = ObjectPath::from(path);
        match self.inner.head(&location).await {
            Ok(meta) => Ok(Some(require_etag(meta.e_tag)?)),
            Err(OsError::NotFound { .. }) => Ok(None),
            Err(e) => Err(store_error(e)),
        }
    }

    async fn put_if_none_match(&self, path: &str, bytes: Vec<u8>) -> Result<WriteOutcome> {
        let location = ObjectPath::from(path);
        let opts = PutOptions::from(PutMode::Create);
        match self.inner.put_opts(&location, bytes.into(), opts).await {
            Ok(res) => Ok(WriteOutcome::Written(require_etag(res.e_tag)?)),
            // Both spellings can surface for a failed create-precondition depending on backend.
            Err(OsError::AlreadyExists { .. }) | Err(OsError::Precondition { .. }) => {
                Ok(WriteOutcome::PreconditionFailed)
            }
            Err(e) => Err(store_error(e)),
        }
    }

    async fn put_if_match(&self, path: &str, bytes: Vec<u8>, etag: &str) -> Result<WriteOutcome> {
        let location = ObjectPath::from(path);
        let version = UpdateVersion {
            e_tag: Some(etag.to_string()),
            version: None,
        };
        let opts = PutOptions::from(PutMode::Update(version));
        match self.inner.put_opts(&location, bytes.into(), opts).await {
            Ok(res) => Ok(WriteOutcome::Written(require_etag(res.e_tag)?)),
            Err(OsError::Precondition { .. }) | Err(OsError::NotModified { .. }) => {
                Ok(WriteOutcome::PreconditionFailed)
            }
            // A vanished object also means our version no longer holds.
            Err(OsError::NotFound { .. }) => Ok(WriteOutcome::PreconditionFailed),
            Err(e) => Err(store_error(e)),
        }
    }
}
