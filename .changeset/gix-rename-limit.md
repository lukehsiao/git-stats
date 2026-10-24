---
"git-stats": patch
---

**fix**: Large commits that rename many files and also edit some of them now count those renames as one changed file each, as `git log --numstat` does. gitoxide used to stop pairing edited renames once the number of candidate pairs passed the rename limit, while git compares that number against the square of the limit. In git.git, `1f010d6bdf75` ("doc: use .adoc extension for AsciiDoc files") went from 993 files and +28,616/-28,616 lines to 919 files and +614/-614, which matches git.
