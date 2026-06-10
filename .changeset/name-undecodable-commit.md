---
"git-stats": patch
---

**fix**: name the offending commit when one fails to decode.

A corrupt commit header used to kill the report with an anonymous "could not read commit data: object parsing failed", leaving no way to locate the bad object in a large history.
When the repository has a commit-graph (git writes one during `gc` by default), the error now reads "could not decode commit <id>".
