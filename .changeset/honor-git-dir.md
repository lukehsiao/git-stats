---
"git-stats": patch
---

**fix**: honor `GIT_DIR` and related environment variables when locating the repository.

Repository discovery only searched upward from the current directory, so `git-stats` failed inside git hooks (git sets `GIT_DIR` for them, and the working directory need not be in the repository) and under `git --git-dir=... stats`.
Discovery now checks the environment first, exactly like git.
