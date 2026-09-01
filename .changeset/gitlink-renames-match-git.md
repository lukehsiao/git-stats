---
"git-stats": patch
---

A renamed submodule now counts as one changed file with no line changes, matching `git log --numstat`, which reports it as `0 0 old => new`.

It previously counted as an addition plus a deletion over two files, because `gitoxide`'s rename tracker did not pair gitlink entries and the two sides arrived as separate changes. Renaming a submodule and repointing it in the same commit still counts as two files with one insertion and one deletion, which is also what git reports.
