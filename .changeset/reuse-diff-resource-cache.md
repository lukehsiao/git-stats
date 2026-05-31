---
"git-stats": patch
---

**perf**: dramatically speed up stats on large repositories.

Per-commit diffing did redundant work that cancelled out parallelism. On a full
clone of `torvalds/linux`, `HEAD~2000..HEAD` (61,860 commits) now runs about 3x
faster, and a full-history run completes in minutes where it was previously
impractical. Output is unchanged.
