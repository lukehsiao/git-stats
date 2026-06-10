---
"git-stats": patch
---

**fix**: support shallow clones.

Since the v0.2.0 rewrite, running `git-stats` in a shallow clone failed with "could not read commit data": computing stats for a commit at the shallow boundary tried to load its parent, which a shallow clone does not have.
Boundary commits now diff against the empty tree, exactly like root commits, matching `git log --numstat`.
Because the truncated history makes every count differ from the full clone's, `git-stats` also prints a warning to stderr when it detects a shallow clone.
