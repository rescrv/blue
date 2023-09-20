bloomcalc
=========

bloomcalc provides a calculator for bloom filters.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library provides the basics of a bloom filter with the bloomcalc command.

```ignore
% target/debug/bloomcalc --prob 0.01
a bloom filter with false positive rate of 0.01 will work best with 6.643856189774724 keys

% target/debug/bloomcalc --card 1000 --prob 0.01
a bloom filter for 1000 items with false positive rate 0.01 will need 9585.058377367439 bits

% target/debug/bloomcalc --card 1000 --bits 10000
a bloom filter for 1000 items with 10000 bits will have a 0.008192549468178963 false positive rate
```

Warts
-----

None.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/bloomcalc/latest/bloomcalc/).
