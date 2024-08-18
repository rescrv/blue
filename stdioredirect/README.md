stdioredirect
=============

stdioredirect is a simple tool to perform redirection of stdio.  It is intended for use as a wrapper program like
numactl or env.

```
USAGE: stdioredirect [--close-$stream|--$stream /file.txt] -- command [args]

Options:
    -h, -help           Print this help menu.
        -close-stdin    "Close stdin. Mutually exclusive with --stdin."
        -close-stdout   "Close stdout. Mutually exclusive with --stdout."
        -close-stderr   "Close stderr. Mutually exclusive with --stderr."
        -stdin          "Redirect stdin to this file in O_RDONLY mode."
        -stdout         "Redirect stdout to this file in O_WRONLY mode,
                        truncating and creating as necessary."
        -stderr         "Redirect stderr to this file in O_WRONLY mode,
                        truncating and creating as necessary."
```

Status
------

Maintenance track.  The binary is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This crate provides the stdioredirect tool.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/stdioredirect/latest/stdioredirect/).
