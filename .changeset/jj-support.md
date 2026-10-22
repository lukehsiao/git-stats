---
"git-stats": minor
---

**feat**: Support jj workspaces, colocated or not. Inside one, `[revision-range]` is a jj revset (change IDs, bookmarks, `trunk()..@`), and it defaults to `::@`, so uncommitted edits in the working-copy commit count.

`git-stats` asks `jj` (0.36 or newer) to evaluate the revset and still reads and diffs the commits with gitoxide. Conflicted commits, and commits on top of them, take their counts from jj's own diff, because jj stores a conflict as a git tree of `.jjconflict-*` directories. `--vcs git` keeps the old behavior in a colocated workspace, for scripts that pass git ranges like `origin..HEAD`.
