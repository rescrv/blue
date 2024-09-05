rc_conf: "
foo_VAR1=variable1\n
"
rc_d: foo
template:
  var1: ${VAR1}
  var2: ${VAR2:?VAR2 is missing}
error: VAR2 is missing
---
rc_conf: "
foo_VAR1=variable1\n
"
rc_d: foo
template:
  ${VAR1}: VAR1
  ${VAR2:?VAR2 is missing}: VAR2
error: VAR2 is missing
