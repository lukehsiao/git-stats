---
"git-stats": patch
---

**perf**: validate `--author`, `--since`, and `--until` before walking the range.

A typo'd pattern or date now errors immediately instead of after a walk that can take minutes on a large history.
