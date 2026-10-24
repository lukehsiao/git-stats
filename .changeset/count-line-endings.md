---
"git-stats": patch
---

**fix**: Line counts now compare lines with their endings, as `git log --numstat` does. A line that gains or loses its final newline now counts as one insertion and one deletion, and a file converted between LF and CRLF counts every line it changes. Before, both counted as no change, so totals could be slightly lower than git's.
