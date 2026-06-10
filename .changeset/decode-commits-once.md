---
"git-stats": patch
---

**perf**: decode each commit header once during the history walk.

The walk used to rescan the raw header separately for the parents, author, committer, and message.
Walk-dominated invocations (heavy filtering, `--reviews`) are about 10% faster; diff-heavy runs are unchanged.
