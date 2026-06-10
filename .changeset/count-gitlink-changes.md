---
"git-stats": patch
---

**fix**: count submodule pointer changes the way `git log --numstat` does.

A gitlink has no blob to diff, so submodule additions and pointer bumps were dropped from the stats entirely.
git renders the pointer as a one-line `Subproject commit <hash>` pseudo-file; `git-stats` now matches it: an added submodule is one file and +1, a repointed one is one file with +1/-1.
A renamed submodule still diverges from git, counting as an addition plus a deletion rather than a paired rename.
