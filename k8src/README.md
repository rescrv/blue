k8src
=====

k8src is the kubernetes rc scripting language.  The high level goal is to implement [Fragmented
Services](https://github.com/rescrv/memcached-rustrc) on top of kubernetes.

I'd like to transform this:

```text
NAMESPACE="memcached"

memcached_IMAGE="rescrv/memcached:latest"
memcached_ENABLED="YES"
memcached_HOST=memcached.example.org
memcached_PORT=11211

memcached_two_INHERIT="YES"
memcached_two_ALIASES="memcached"
memcached_two_PORT="22122"

VALUES_METRO="//metros.conf"
VALUES_CUSTOMER="//customers.conf"

FILTER_METRO_CUSTOMER="//metros-customers.conf"

METRO_CUSTOMER_memcached_AUTOGEN="YES"
METRO_CUSTOMER_memcached_ENABLED="YES"
METRO_CUSTOMER_memcached_ALIASES="memcached"
METRO_CUSTOMER_memcached_INHERIT="YES"
METRO_CUSTOMER_memcached_HOST="${CUSTOMER}.${METRO}.memcached.example.org"

# Perhaps this is a legacy setup from before the fragmenting.
Jfk_PlanetExpress_memcached_HOST="planetexpress.example.org"
Jfk_PlanetExpress_memcached_PORT="4242"
```

to a set of kubernetes manifests that deploy one memcached host per customer.  For SaaS apps that are partitioned by
customer, this pattern enables easy turn-up and turn-down of customer-oriented services.  That's what I wanted.

Features:

- Flexible generation of YAML ensures that every template is customizable.
- Dynamic interfaces for configuration allow containers to declare which environment variables influence their behavior.
  k8src will automatically populate these variables from rc.conf.
- Service aliasing allows one set of configs and one image to be built to serve multiple deployments.
- Fragmented services allow deployment of one instance of the application per customer or per (customer X metro) and get
  isolation between components.

How it Works
------------

At its core, k8src is simply a shell-like substitution library for YAML.  Given an `rc.conf`, it will substitute all
values according to the cascading rules of rc into the YAML.

For example, here's a simple template for the `memcached` service above:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${SERVICE:?SERVICE not set}
  namespace: ${NAMESPACE:?NAMESPACE not set}
  labels:
    app: ${SERVICE}
spec:
  replicas: ${REPLICAS:?}
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

This will "do the right thing" and substitutes the variables above.  There's just a few things to call out:
- The syntax matches that of FreeBSD's `/bin/sh` parameter expansion for `${FOO:-expand if not set}` `${FOO:?ERROR
  message}` and `${FOO:+expand if set}`.
- If a takes the form `${FOO:?}`, it's not an error, but an optional value.  Optional values will be omitted in a
  cascading fashion (up to an empty container).

Input Format
------------

```text
/rc.conf
/metros.conf
/customers.conf
/metros-customers.conf
/templates
/templates/service.yaml.template
/templates/rc.d
/templates/rc.d/memcached.yaml.template
/pets/...
```

This is one top level declaration, similar to a k8s kustomize variant.  This one rc.conf will be used to generate a
manifest.  For anything that aliases to `memcached`, whether directly or transitively, the memcached.yaml.template will
be used to generate a single file matching the template in the output hierarchy.

Output Format
-------------

The example inputs above yield the following output:

```text
/kustomization.yaml
/herd
/herd/Jfk_PlanetExpress_memcached.yaml
/herd/Jfk_TyrellCorp_memcached.yaml
/herd/kustomization.yaml
/herd/memcached_two.yaml
/herd/memcached.yaml
/herd/Sac_Acme_memcached.yaml
/herd/Sfo_ApertureScience_memcached.yaml
/herd/Sjc_CyberDyne_memcached.yaml
/pets/...
```

Notice that we get one output file for each valid (metro, customer) combination.  k8src puts all services that come from
rc.conf aliases in the herd directory.  The pets directory should be valid customize and will be copied verbatim.

Overlays
--------

Imagine we had the following directory structure:

```text
/rc.conf
/templates/rc.d/Sjc_CyberDyne_memcached.yaml.template
/env1/rc.conf
/env1/templates/service.yaml.template
/env2/rc.conf
```

In this case, k8src will generate manifests for terminal rc.conf files.  It will automatically infer the `rc_conf_path`
`rc.conf:env1/rc.conf`, where later values mask earlier values.  Thus env1 could be mostly the same as the base, but
with one or two added lines.  k8src will not generate manifests for overlays in parent directories of rc.conf files.
The templates will be resolved starting with the deepest directory first.  The (Sjc, CyberDyne) service will be
specialized in env1 and env2, and the service.yaml.template provided in env1 will apply as the default for env1 only.

Running k8src
-------------

```console
$ k8src regenerate --help
```

Status
------

Active development.  I plan to build tooling for rolling out rc.conf changes and then mark it as maintenance track.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/k8src/latest/k8src/).
