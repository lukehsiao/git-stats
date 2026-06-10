---
"git-stats": patch
---

**fix**: hide every merge base in symmetric difference (`A...B`) ranges.

`A...B` hid only the single "best" merge base, but criss-cross histories have more than one.
Commits reachable from the unhidden bases leaked into the walk, inflating the counts relative to `git rev-list --count A...B`.
All merge bases are now hidden, matching git.
