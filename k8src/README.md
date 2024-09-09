k8src
=====

k8src is the kubernetes rc scripting language.  The high level goal is to implement [Fragmented
Services](https://github.com/rescrv/memcached-rustrc) on top of kubernetes.

I'd like to transform this:

```text
some_stateless_service_IMAGE="some_stateless_service:2024-09-05"
some_stateless_service_ENABLED="YES"

mpd_IMAGE="mpd:latest"
mpd_ENABLED="YES"
mpd_PORT=6600

smtpd_IMAGE="smtpd:stable"
smtpd_ENABLED="YES"
smtpd_PORT=587
smtpd_REPLICAS=2
```

to a set of kubernetes manifests.

At its core, k8src is simply a shell-like substitution library for YAML.  Given an `rc.conf`, it will substitute all
values according to the cascading rules of rc into the YAML.

For example, here's a simple template for the `some_stateless_service` service above:

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
- The syntax matches that of FreeBSD's `/bin/sh` parameter expansion for `${FOO:-default}` `${FOO:?ERROR message}` and
  `${FOO:+expand if set}`.
- If a takes the form `${FOO:?}`, it's not an error, but an optional value.  Optional values will be cascadingly
  omitted.

The output format is under development as I learn k8s.

Status
------

Exploratory.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/k8src/latest/k8src/).
