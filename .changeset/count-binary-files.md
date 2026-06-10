---
"git-stats": patch
---

**fix**: count binary file changes in the Changed Files column.

Since the v0.2.0 rewrite, a binary file change was dropped entirely from the stats because it has no line counts, under-reporting the Changed Files column for commits touching images or other binary blobs.
Binary changes now count as changed files contributing no lines, matching `git diff --shortstat`.
