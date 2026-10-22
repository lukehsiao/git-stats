---
"git-stats": patch
---

**fix**: `--sort author` now lists authors A to Z, as its help text always said, and `--reverse` gives Z to A. It used to print Z to A by default. The help for `--sort` and `--reverse` now also says that they order only the stats table; the reviewers table always sorts by commits.
