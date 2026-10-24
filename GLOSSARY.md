# git-stats

git-stats summarizes who changed a repository's history and who reviewed it, in the spirit of `git shortlog` with line counts.

## Language

### People

**Contributor**:
A person credited in a table, either as an Author or as a Reviewer.
_Avoid_: user, person, committer

**Author**:
A Contributor credited for writing a commit, after `.mailmap` resolution.
_Avoid_: committer

**Reviewer**:
A Contributor named in a commit's `Acked-by`, `Tested-by`, or `Reviewed-by` trailer.
_Avoid_: approver, signer

**Contributor key**:
The label that identifies a Contributor's row: the name, or `Name <email>` with `--email`. An empty name uses the email instead, and a missing email leaves just the name. Contributors with the same key share one row.
_Avoid_: author key, reviewer key, ident

### Reports

**Stats table**:
The table of per-Author commit, file, and line counts, followed by the Total row.
_Avoid_: author table, summary

**Reviews table**:
The table of per-Reviewer commit counts.
_Avoid_: reviewers table, sign-off table

**Total row**:
The last row of the Stats table, which sums every Author row.
_Avoid_: summary row, footer

**Numstat**:
A commit's changed-file, insertion, and deletion counts, as `git log --numstat` reports them.
_Avoid_: diffstat, line counts

**Revision range**:
The commits a report covers: a git revision range, or a revset in a jj workspace.
_Avoid_: range spec, commit range
