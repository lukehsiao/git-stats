---
"git-stats": patch
---

**fix**: A commit whose tree cannot be read now fails with an error that names the commit. Before, the error said only "could not read commit data" or "could not compute diff stats".
