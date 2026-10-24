---
"git-stats": patch
---

**fix**: A file swapped with a submodule now counts as one changed file, with the file's lines on one side and the `Subproject commit` line on the other, as `git log --numstat` counts it. Before, the change counted as nothing. A file whose contents cannot be read now fails the run instead of counting as nothing, and in a partial clone (`git clone --filter=blob:none`) the error says the clone lacks objects and how to fetch them. Before, a fresh partial clone silently undercounted, counting only changes whose contents were present, or failed with an unexplained "could not compute diff stats" error.
