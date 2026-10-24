---
"git-stats": patch
---

**fix**: Reviewers now go through `.mailmap`, as `git shortlog --group=trailer` does, so a reviewer who changed name or email is one row instead of two. Trailer values are also split the way git splits them, which shows with `--email`: the email ends at the first `>`, so `Ada <ada@example.com> # see <foo>` is keyed as `Ada <ada@example.com>`, and spaces inside the brackets are kept, so `Ada < ada@example.com >` no longer merges with `Ada <ada@example.com>`.
