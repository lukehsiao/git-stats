---
"git-stats": patch
---

**refactor**: Rows that tie in the sorted column, in both the stats and the reviewers tables, now sort by name, A to Z. They used to keep the order in which the history walk first saw each author, so the same commits could print in a different order under git and jj.
