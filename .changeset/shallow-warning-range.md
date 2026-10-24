---
"git-stats": patch
---

**fix**: The shallow-clone warning now appears only when the range reaches the shallow boundary. Before, it appeared for every range in a shallow clone, even one like `HEAD~3..HEAD` whose commits are all present. A revision that does not resolve in a shallow clone now fails with an error that says the clone may lack it.
