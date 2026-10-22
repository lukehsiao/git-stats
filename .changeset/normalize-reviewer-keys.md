---
"git-stats": patch
---

**fix**: Reviewer rows now merge trailers that differ only in whitespace, such as `Ada  <ada@example.com>` and `Ada <ada@example.com>` under `--email`, or `Ada<ada@example.com>` without it. A trailer with only `<ada@example.com>` now shows the email instead of the raw `<...>` text.
