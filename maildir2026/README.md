maildir2026
===========

maildir2026 provides a small Maildir-style file handoff protocol.

Writers create complete files in `tmp` and publish them by moving them to `new`.
Readers claim files by moving them from `new` to `cur`.
