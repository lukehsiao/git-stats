---
"git-stats": patch
---

**fix**: peel annotated tags to commits in revision ranges.

Since the v0.2.0 rewrite, range endpoints naming annotated tags were not peeled
the way git peels them at rev-list endpoints. An annotated tag on the excluded
side (`tag..HEAD`) silently returned whole-repo history with exit 0, while a
tag on the included side errored with "Expected object of kind commit but got
tag". Lightweight tags were unaffected. This bit our own release notes: the
v0.2.0 and v0.2.1 stats tables claimed 181 and 184 commits for 9- and 3-commit
releases.
