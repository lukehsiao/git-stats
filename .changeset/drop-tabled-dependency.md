---
"git-stats": patch
---

Drop the `tabled` dependency and lay out the two tables directly.

It pulled in 12 transitive crates, one of them the unmaintained `proc-macro-error2` (RUSTSEC-2026-0173), to produce a borderless table with two-space column gaps. We can do that ourselves. The output remains byte-identical.
