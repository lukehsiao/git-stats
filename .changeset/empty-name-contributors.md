---
"git-stats": patch
---

**fix**: An author with an empty name is now shown by their email, as reviewers already were, instead of a blank row (or ` <email>` with `--email`). This also splits rows: without `--email`, all empty-name authors used to share one blank row, and now each email gets its own. `git shortlog` still groups them under one blank name. An author without an email now shows as just the name with `--email`, as reviewers already did, rather than `Name <>`.
