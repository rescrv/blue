arrrg
=====

arrrg provides an opinionated [CommandLine] parser.

For example, let's consider the parser specified here using the derive syntax:

```
use arrrg_derive::CommandLine;

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct Options {
    #[arrrg(optional, "this is the help text", "PLACEHOLDER")]
    some_string: String,
    #[arrrg(nested)]
    some_prefix: SomeOptions,
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct SomeOptions {
    #[arrrg(required, "this is the help text", "PLACEHOLDER")]
    a: String,
    #[arrrg(optional, "this is the help text", "PLACEHOLDER")]
    b: String,
}
```

This will provide the options to getopts of `--some-string`, `--some-prefix-a`,
`--some-prefix-b`.  In general the rule is to derive the flag names from the identifiers of
struct members.  When nesting the name will be the concatenation of the prefix from the parent
struct and the member identifier from the child struct.  Unlimited nesting is possible.
Underscores change to dashes.

This library takes an opinionated stance on the command line.  There should be exactly one
canonical argument order on the command-line and all applications must be built with this in
mind.  Users of the library can call [CommandLine::from_command_line_relaxed] to disable this checking.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

arrrg will provide the CommandLine trait and wrapper around getopts.

Warts
-----

- Nested derive parameter names can get unwieldy.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/arrrg/latest/arrrg/).
