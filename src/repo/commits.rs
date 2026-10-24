//! Commit reading: what the walk learns about each selected commit, from either adapter.

use super::{Numstat, WalkedCommit};
use crate::error::{Error, Result};
use crate::model::{CommitMeta, Contributor, DiffStat};

/// Read the metadata of each commit in `commits`, keeping their order. Each comes with its numstat
/// when that is already known.
pub(super) fn read(
    repo: &gix::Repository,
    commits: impl Iterator<Item = Result<(gix::ObjectId, Option<DiffStat>)>>,
    need_reviewers: bool,
) -> Result<Vec<WalkedCommit>> {
    let mailmap = repo.open_mailmap();
    // git grafts commits at the shallow boundary as parentless: one whose header names several
    // parents is not a merge there, and it diffs against the empty tree. An unreadable shallow file
    // degrades to "not shallow", so a boundary commit then surfaces its missing parent as a read
    // error when its numstat is computed.
    let shallow = repo.shallow_commits().ok().flatten();
    commits
        .map(|item| {
            let (id, known) = item?;
            let commit = repo
                .find_commit(id)
                .map_err(|e| Error::ReadCommit(Box::new(e)))?;
            // Decode the header once; the author, committer, parents, and message accessors on
            // `gix::Commit` would each rescan the raw bytes.
            let commit = commit.decode().map_err(|e| Error::DecodeCommit {
                id: id.to_string(),
                source: e.into(),
            })?;
            let is_boundary = shallow
                .as_ref()
                .is_some_and(|s| s.binary_search(&id).is_ok());
            // A merge adds nothing, as `git log --numstat` reports it. jj counts a merge against
            // its auto-merged parents, which git never does, so a grafted merge at the shallow
            // boundary diffs its whole tree like any other boundary commit. A numstat from jj wins
            // over the boundary rule: jj diffs a boundary commit against its virtual root commit,
            // which is the same whole-tree diff, and only jj sees past a conflict's `.jjconflict-*`
            // trees.
            let is_merge = commit.parents.len() > 1;
            let numstat = if is_merge && !is_boundary {
                Numstat::Known(DiffStat::default())
            } else if let Some(diff) = known.filter(|_| !is_merge) {
                Numstat::Known(diff)
            } else {
                let base = if is_boundary {
                    None
                } else {
                    commit.parents().next()
                };
                Numstat::Diff { id, base }
            };
            Ok(WalkedCommit {
                meta: commit_meta(&commit, &mailmap, need_reviewers)?,
                numstat,
            })
        })
        .collect()
}

fn commit_meta(
    commit: &gix::objs::CommitRef<'_>,
    mailmap: &gix::mailmap::Snapshot,
    need_reviewers: bool,
) -> Result<CommitMeta> {
    let author = mailmap.resolve(commit.author().map_err(|e| Error::ReadCommit(e.into()))?);
    let time_seconds = commit
        .committer()
        .map_err(|e| Error::ReadCommit(e.into()))?
        .seconds();
    let reviewers = if need_reviewers {
        parse_reviewers(commit, mailmap)
    } else {
        Vec::new()
    };
    Ok(CommitMeta {
        author: Contributor {
            name: author.name.to_string(),
            email: author.email.to_string(),
        },
        time_seconds,
        reviewers,
    })
}

/// Trailer tokens that name a Reviewer, compared without regard to ASCII case as git compares them.
const REVIEW_TOKENS: [&str; 3] = ["Acked-by", "Tested-by", "Reviewed-by"];

/// The Reviewers that a commit's trailers name, one per review trailer, in message order. Other
/// trailers, such as `Signed-off-by`, are skipped before any parsing or mailmap lookup.
fn parse_reviewers(
    commit: &gix::objs::CommitRef<'_>,
    mailmap: &gix::mailmap::Snapshot,
) -> Vec<Contributor> {
    let Some(body) = commit.message().body() else {
        return Vec::new();
    };
    body.trailers()
        .filter(|t| {
            REVIEW_TOKENS
                .iter()
                .any(|k| t.token.eq_ignore_ascii_case(k.as_bytes()))
        })
        .map(|t| trailer_contributor(&t.value.to_string(), mailmap))
        .collect()
}

/// The Contributor a trailer value names, resolved through `.mailmap` as `git shortlog
/// --group=trailer` does. A value that is not `Name <email>` is taken literally as the name, with
/// no email and no mailmap lookup, as git takes it.
fn trailer_contributor(value: &str, mailmap: &gix::mailmap::Snapshot) -> Contributor {
    let Some((name, email)) = split_ident(value) else {
        return Contributor {
            name: value.to_string(),
            email: String::new(),
        };
    };
    // The mailmap matches on name and email only, so the signature needs no time.
    let resolved = mailmap.resolve_cow(gix::actor::SignatureRef {
        name: name.into(),
        email: email.into(),
        time: "",
    });
    Contributor {
        name: resolved.name.to_string(),
        email: resolved.email.to_string(),
    }
}

/// Split `Name <email>` as git's `split_ident_line` does. The name is everything before the first
/// `<`, without trailing whitespace. The email is everything between that `<` and the first `>`
/// after it, untrimmed. Anything after that `>` is ignored. Returns `None` without a `<`, or
/// without a `>` after it.
fn split_ident(value: &str) -> Option<(&str, &str)> {
    // git's `isspace` is locale-independent and covers only these four characters, so a trailing
    // no-break space or vertical tab stays part of the name.
    const GIT_SPACE: [char; 4] = [' ', '\t', '\n', '\r'];
    let (name, rest) = value.split_once('<')?;
    let (email, _) = rest.split_once('>')?;
    Some((name.trim_end_matches(GIT_SPACE), email))
}

#[cfg(test)]
mod tests {
    use hegel::generators;

    use super::{parse_reviewers, split_ident, trailer_contributor};
    use crate::model::Contributor;

    /// Each case is how git's `split_ident_line` splits the value, so the Reviewer row matches `git
    /// shortlog --group=trailer`.
    #[test]
    fn split_ident_follows_git() {
        let cases = [
            ("Ada Lovelace <ada@x>", Some(("Ada Lovelace", "ada@x"))),
            ("<ada@x>", Some(("", "ada@x"))),
            ("Ada  \t<ada@x>", Some(("Ada", "ada@x"))),
            ("Ada<ada@x>", Some(("Ada", "ada@x"))),
            ("Ada < ada@x >", Some(("Ada", " ada@x "))),
            ("Ada <ada@x> # see <foo>", Some(("Ada", "ada@x"))),
            ("Ada <>", Some(("Ada", ""))),
            ("Ada\r\n <ada@x>", Some(("Ada", "ada@x"))),
            ("Ada\u{a0} <ada@x>", Some(("Ada\u{a0}", "ada@x"))),
            ("Ada\u{b} <ada@x>", Some(("Ada\u{b}", "ada@x"))),
            ("Ada", None),
            ("Ada <ada@x", None),
            ("Ada ada@x>", None),
        ];
        for (value, want) in cases {
            assert_eq!(split_ident(value), want, "value: {value:?}");
        }
    }

    /// Any `Name <email>` splits back into its parts, whatever follows the `>`.
    #[hegel::test]
    fn split_ident_recovers_name_and_email(tc: hegel::TestCase) {
        let name = tc.draw(generators::from_regex(r"([^<\s]([^<]*[^<\s])?)?").fullmatch(true));
        let email = tc.draw(generators::from_regex(r"[^>]*").fullmatch(true));
        let gap = tc.draw(generators::from_regex(r"[ \t]*").fullmatch(true));
        let tail = tc.draw(generators::text());
        let value = format!("{name}{gap}<{email}>{tail}");
        assert_eq!(split_ident(&value), Some((name.as_str(), email.as_str())));
    }

    #[test]
    fn trailer_contributors_resolve_through_the_mailmap() {
        let mailmap = gix::mailmap::Snapshot::from_bytes(b"New Name <new@x> <old@x>\n");
        let who = |name: &str, email: &str| Contributor {
            name: name.to_string(),
            email: email.to_string(),
        };
        assert_eq!(
            trailer_contributor("Old Name <old@x>", &mailmap),
            who("New Name", "new@x")
        );
        assert_eq!(
            trailer_contributor("Other <other@x>", &mailmap),
            who("Other", "other@x")
        );
        // git takes a value that is not an identity literally, without a mailmap lookup.
        assert_eq!(trailer_contributor("old@x", &mailmap), who("old@x", ""));
    }

    /// Only review trailers name Reviewers, whatever their case, one entry per trailer in message
    /// order.
    #[test]
    fn parse_reviewers_keeps_only_review_trailers() {
        let raw = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
            author A <a@x> 1600000000 +0000\n\
            committer A <a@x> 1600000000 +0000\n\
            \n\
            subject\n\
            \n\
            body\n\
            \n\
            Signed-off-by: A <a@x>\n\
            reviewed-BY: Rev <rev@x>\n\
            Tested-by: Rev <rev@x>\n\
            Acked-by: Ack <ack@x>\n";
        let commit = gix::objs::CommitRef::from_bytes(raw, gix::hash::Kind::Sha1).unwrap();
        let names: Vec<String> = parse_reviewers(&commit, &gix::mailmap::Snapshot::default())
            .into_iter()
            .map(|c| c.ident())
            .collect();
        assert_eq!(names, ["Rev <rev@x>", "Rev <rev@x>", "Ack <ack@x>"]);
    }
}
