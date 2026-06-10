---
"git-stats": patch
---

**fix**: drop the stray leading blank line when only the reviews table renders.

Reviews ignore the author and date filters by design, so filters that match no commits leave the stats table empty while `--reviews` still prints.
The output began with a blank line meant to separate the two tables; it now appears only when both tables are present.
