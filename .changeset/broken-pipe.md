---
"git-stats": patch
---

**fix**: exit quietly when stdout closes early instead of panicking.

Piping into a pager or `head` closes stdout before the report is fully written.
Rust ignores SIGPIPE, so `git-stats | head` could die with a noisy "failed printing to stdout: Broken pipe" panic.
It now exits silently with status 141, the same status a shell reports for git itself.
