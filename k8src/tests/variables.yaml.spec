rc_conf: "
foo_VAR1=variable1\n
foo_VAR2=variable2
"
rc_d: foo
template:
  var1: ${VAR1}
  var2: ${VAR2}
expected:
  var1: variable1
  var2: variable2
---
rc_conf: "
foo_VAR1=variable1\n
foo_VAR2=variable2
"
rc_d: foo
template:
  ${VAR1}: VAR1
  ${VAR2}: VAR2
expected:
  variable1: VAR1
  variable2: VAR2
