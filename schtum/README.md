schtum
======

schtum keeps secrets.  It holds named rings of versioned key material and rotates them on a
schedule written into the manifest, so every holder flips on the same wall-clock instant with no
coordination beyond a shared clock.  arrrg and rc_conf carry configuration; schtum carries what
must not be in configuration.

Two consumers, one structure:

- Client credentials: one valid at a time; the old one is invalidated at a third party after the
  new one is in use.
- Symmetric keys between internal systems: the root key itself rotates, and every artifact
  produced under it (macaroons, MACs) stays verifiable until it expires.

Core model
----------

```text
Domain   = { written_at, rings }
Ring     = { name, max_artifact_ttl, versions }
Version  = { id, material, primary_from, accept_until }
```

State is derived from the clock, never stored: a version is *staged* before `primary_from`,
*primary* if it is the latest whose `primary_from` has passed, *retired* until `accept_until`, and
*dead* after.  Rotation is one write:

```text
vN+1.primary_from = now + lag
vN.accept_until   = vN+1.primary_from + max_artifact_ttl
```

The only dangerous lag is a holder learning of vN+1 after `primary_from`; it then rejects peers'
artifacts as `Lookup::Unknown`, which is its own sensor.  Holders learning of vN's death late is
harmless.

Quick start
-----------

```rust
use schtum::{Manifest, Material, Ring, VersionId};

let mut manifest = Manifest::new();
let mut ring = Ring::new("macaroon-root", 24 * 3600)?;
ring.rotate(schtum::now(), 300, Material::random(32)?)?;
manifest.add_ring(ring)?;
manifest.write_dir(&utf8path::Path::from("/tmp/schtum-example"), schtum::now())?;

let domain = schtum::Domain::open("/tmp/schtum-example")?;
let _watcher = domain.watch(std::time::Duration::from_secs(1));
let ring = domain.ring("macaroon-root").unwrap();
// A staged version is accepted but not yet produced under; primary() errs until primary_from.
assert!(ring.get(VersionId(1)).is_ok());
# std::fs::remove_dir_all("/tmp/schtum-example").ok();
# Ok::<_, schtum::Error>(())
```

Every artifact carries its kid, `(ring, VersionId)`; consumers look up by kid and never
trial-verify.  For macarunes, encode the kid in the (opaque) identifier and build the
`macarunes::Secret` with `Secret::try_from(material.expose())`.

On disk and in Kubernetes
-------------------------

A domain is a directory: `_domain` holds the header (`written_at`, ring names); each ring is a file
named for it.  A Kubernetes Secret is the same thing with `data` keys for files, so a trust domain
is a Secret and rings within it share RBAC.  Mount it as a volume — environment variables and
`subPath` mounts never update, which silently breaks rotation.

The `schtum` binary edits directories (`dir:PATH`) and Secrets (`k8s:NAMESPACE/NAME`, via
`kubectl`) with read-CAS-write on `resourceVersion`:

```console
$ schtum init dir:/tmp/secrets
$ schtum add-ring --ttl 24h dir:/tmp/secrets macaroon-root
$ schtum rotate --lag 5m dir:/tmp/secrets macaroon-root
$ schtum show dir:/tmp/secrets
$ schtum gc --dry-run dir:/tmp/secrets
```

Biometrics
----------

`schtum.domain.written_at` is the fleet-wide lag reference: a holder reporting a value below the
one the CLI just wrote has not loaded the rotation.  `schtum.staged.count` must be nonzero on every
holder immediately before a `primary_from`.  `schtum.verify.unknown_kid` is the outage signal.
The CLI writes and exits; convergence is the alerting system's job.

Status
------

Active development.

Scope
-----

This crate provides the ring model, the on-disk domain format, a reloading `Domain` with watcher,
`SecretRef` for configuration, and the `schtum` CLI.

Warts
-----

- Material lives in a `Vec<u8>`; the buffer is scrubbed on drop, but nothing prevents a
  reallocation from leaving a stale copy behind.  Material is never resized after construction.
- Sensors are process-wide.  A process that opens several domains reports the last-reloaded
  domain's `written_at`.
- The Kubernetes store shells out to `kubectl` and has been tested against fixtures, not a cluster.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/schtum/latest/schtum/).
