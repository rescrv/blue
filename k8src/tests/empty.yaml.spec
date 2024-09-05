rc_conf: '
'
rc_d: foo
template:
expected:
---
rc_conf: '
'
rc_d: foo
template:
    foo:
expected:
    foo:
---
rc_conf: '
'
rc_d: bar
template:
    foo:
expected:
    foo:
