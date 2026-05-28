k8src
=====

k8src is the kubernetes rc scripting language.  It combines `rc.conf` style
configuration with YAML templates to generate Kubernetes manifests.

The common layout is:

```text
/rc.conf
/service.yaml.template
/rc.d/<service>.yaml.template
/pets/...
```

`service.yaml.template` is the default template for enabled services.  A file in
`rc.d/` with the service name overrides the default template for that service and
for aliases that resolve to it.  `pets/` is copied through to the generated
manifests unchanged.

Quick Start
-----------

Create a minimal project:

```console
$ k8src init
$ k8src regenerate --dry-run
$ k8src regenerate --overwrite
```

Print the built-in service template:

```console
$ k8src template service.yaml.template
```

Inspect what k8src will do:

```console
$ k8src explain-template memcached
$ k8src explain-vars memcached
$ k8src regenerate --diff
```

How it Works
------------

At its core, k8src is a shell-like substitution library for YAML.  Given an
`rc.conf`, it substitutes values according to the cascading rules of rc into the
YAML.

For example, this `rc.conf` enables a memcached service:

```text
NAMESPACE="memcached"

memcached_IMAGE="rescrv/memcached:latest"
memcached_ENABLED="YES"
memcached_HOST=memcached.example.org
memcached_PORT=11211

memcached_two_INHERIT="YES"
memcached_two_ALIASES="memcached"
memcached_two_ENABLED="YES"
memcached_two_PORT="22122"
```

A matching `service.yaml.template` can use those values:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${SERVICE:?SERVICE not set}
  namespace: ${NAMESPACE:?NAMESPACE not set}
  labels:
    app: ${SERVICE}
spec:
  replicas: ${REPLICAS:-1}
  selector:
    matchLabels:
      app: ${SERVICE}
  template:
    metadata:
      labels:
        app: ${SERVICE}
    spec:
      containers:
      - name: ${SERVICE}
        image: ${IMAGE:?IMAGE not set}
        ports:
        - containerPort: ${PORT:-8000}
---
apiVersion: v1
kind: Service
metadata:
  name: ${SERVICE}
  namespace: ${NAMESPACE}
  labels:
    app: ${SERVICE}
spec:
  type: ClusterIP
  ports:
  - port: ${PORT:-8000}
    protocol: TCP
    targetPort: ${PORT:-8000}
  selector:
    app: ${SERVICE}
```

The syntax matches FreeBSD `/bin/sh` parameter expansion for
`${FOO:-expand if not set}`, `${FOO:?ERROR message}`, and
`${FOO:+expand if set}`.  `${FOO:?}` marks a value optional; optional values are
omitted in a cascading fashion.

Template Selection
------------------

For each enabled service, k8src searches from the deepest overlay back to the
root:

```text
<overlay>/rc.d/<service>.yaml.template
<root>/rc.d/<service>.yaml.template
<overlay>/service.yaml.template
<root>/service.yaml.template
built-in default template
```

Alias fallback is transitive.  If `frontend` aliases `app`, k8src first looks for
`rc.d/frontend.yaml.template`, then for `rc.d/app.yaml.template`, then for the
default template.

Output Format
-------------

Generated output goes under `manifests/`:

```text
/manifests/kustomization.yaml
/manifests/herd
/manifests/herd/kustomization.yaml
/manifests/herd/memcached.yaml
/manifests/herd/memcached_two.yaml
/manifests/pets/...
```

Services from `rc.conf` are generated under `herd/`.  Files under `pets/` should
already be valid kustomize input and are copied verbatim.

Overlays
--------

Overlays are nested directories with their own `rc.conf`.  For example:

```text
/rc.conf
/service.yaml.template
/rc.d/Sjc_CyberDyne_memcached.yaml.template
/pets/...
/env1/rc.conf
/env1/service.yaml.template
/env1/rc.d/memcached.yaml.template
/env1/pets/...
/env2/rc.conf
```

k8src generates manifests for terminal `rc.conf` files.  It infers an
`rc_conf_path` such as `rc.conf:env1/rc.conf`, where later values mask earlier
values.  Each overlay may override `env/rc.conf`,
`env/service.yaml.template`, `env/rc.d/<service>.yaml.template`, and
`env/pets/...`.

Running k8src
-------------

```console
$ k8src help
$ k8src regenerate --help
```

Status
------

Active development.  I plan to build tooling for rolling out `rc.conf` changes
and then mark it as maintenance track.

Documentation
-------------

The latest documentation is always available at
[docs.rs](https://docs.rs/k8src/latest/k8src/).
