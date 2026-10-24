//! The jj adapter: the `jj` CLI evaluates a revset in a workspace, and gix reads the commits it
//! selects from the git store that backs the workspace.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::Selected;
use crate::error::{Error, Result};
use crate::model::DiffStat;

/// Open the git repository that backs the jj workspace rooted at `workspace`.
pub(super) fn open(workspace: &Path) -> Result<gix::ThreadSafeRepository> {
    // Reading the store location needs no snapshot of the working copy. The revset selection
    // snapshots it later, before any commit is read.
    let git_dir = run(
        workspace,
        "git root",
        &["git", "root", "--ignore-working-copy"],
    )?;
    let git_dir = git_dir.strip_suffix('\n').unwrap_or(&git_dir);
    let mut repo = gix::open(git_dir).map_err(|e| Error::OpenRepository(Box::new(e)))?;
    // gix reads `.mailmap` and `.gitattributes` from the worktree of the git store. jj's own store
    // has none, and a colocated or external store's worktree may be another workspace. So the
    // workspace stands in for the worktree, as the checkout does for git.
    repo.set_workdir(workspace.to_path_buf())
        .map_err(|e| Error::OpenRepository(Box::new(e)))?;
    Ok(repo.into_sync())
}

/// Select every commit in the jj `revset`, which defaults to `::@`, with jj's numstat for each
/// commit that [`LOG_TEMPLATE`] gives one.
pub(super) fn select(workspace: &Path, revset: Option<&str>) -> Result<Selected<'static>> {
    // Like any jj command, `jj log` snapshots the working copy first, so `@` includes edits that
    // are not yet committed elsewhere.
    let out = run(
        workspace,
        "log",
        &[
            "log",
            "--no-graph",
            "--revisions",
            revset.unwrap_or("::@"),
            "--template",
            LOG_TEMPLATE,
        ],
    )?;
    Ok(Box::new(parse_log(&out)?.into_iter().map(Ok)))
}

/// Parse the output of `jj log` with [`LOG_TEMPLATE`]: each commit in order, with jj's numstat
/// when it printed one. A line that does not parse is an error.
fn parse_log(out: &str) -> Result<Vec<(gix::ObjectId, Option<DiffStat>)>> {
    out.lines()
        .filter_map(|line| match parse_log_line(line) {
            // jj's `root()` is a virtual commit with an all-zero ID and no git object to read.
            Some((id, _)) if id.is_null() => None,
            Some(commit) => Some(Ok(commit)),
            None => Some(Err(Error::JjOutput {
                command: "log",
                output: line.to_string(),
            })),
        })
        .collect()
}

/// The `jj log` template for [`select`]: one line per commit with its ID. A commit that is
/// conflicted, or has a conflicted parent, also gets its file, insertion, and deletion counts. jj
/// stores a conflict as a git tree of `.jjconflict-*` directories, so a git diff that touches one
/// would count those directories rather than the change. jj diffs only these commits. Its diff is
/// single-threaded and, on a 12,000-commit history, roughly ten times slower than the parallel gix
/// diff.
const LOG_TEMPLATE: &str = r#"
commit_id ++ if(
  conflict || parents.any(|p| p.conflict()),
  " " ++ self.diff().stat().files().len()
    ++ " " ++ self.diff().stat().total_added()
    ++ " " ++ self.diff().stat().total_removed()
) ++ "\n"
"#;

/// Parse one line printed by [`LOG_TEMPLATE`], without its newline.
fn parse_log_line(line: &str) -> Option<(gix::ObjectId, Option<DiffStat>)> {
    let mut fields = line.split(' ');
    let id = gix::ObjectId::from_hex(fields.next()?.as_bytes()).ok()?;
    let diff = match [fields.next(), fields.next(), fields.next(), fields.next()] {
        [None, ..] => None,
        [Some(files), Some(insertions), Some(deletions), None] => Some(DiffStat {
            files: files.parse().ok()?,
            insertions: insertions.parse().ok()?,
            deletions: deletions.parse().ok()?,
        }),
        _ => return None,
    };
    Some((id, diff))
}

/// A diff resource cache with the `.gitattributes` of the workspace. The index and `HEAD` of jj's
/// git store may be empty, or may belong to another workspace, so attributes come only from the
/// files in this workspace, which [`open`] made the worktree.
pub(super) fn diff_cache(repo: &gix::Repository) -> Result<gix::diff::blob::Platform> {
    let err = |e: gix::Error| Error::DiffSetup(Box::new(e));
    let index = gix::index::State::new(repo.object_hash());
    let attributes = repo
        .attributes_only(
            &index,
            gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
        )
        .map_err(err)?
        .detach();
    gix::diff::resource_cache(
        repo,
        gix::diff::blob::pipeline::Mode::ToGit,
        attributes,
        gix::diff::blob::pipeline::WorktreeRoots::default(),
    )
    .map_err(err)
}

/// The root of the jj workspace containing `path`: the nearest ancestor with a `.jj` directory.
/// With `stop_at_git`, a nearer `.git` ends the search, so a git repository nested inside a jj
/// workspace stays git. A colocated workspace has both at its root and counts as jj.
pub(super) fn find_workspace(path: &Path, stop_at_git: bool) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    for dir in path.ancestors() {
        if dir.join(".jj").is_dir() {
            return Some(dir.to_path_buf());
        }
        if stop_at_git && dir.join(".git").exists() {
            return None;
        }
    }
    None
}

/// Run `jj` in `workspace` and return its stdout. `command` names the subcommand in errors. jj's
/// stderr passes through, so its own errors, hints, and warnings reach the user verbatim.
fn run(workspace: &Path, command: &'static str, args: &[&str]) -> Result<String> {
    let out = Command::new("jj")
        .current_dir(workspace)
        // Keep stdout machine-readable whatever the user's jj config says. A wrapped line would
        // split a commit's fields across two lines.
        .args([
            "--no-pager",
            "--color=never",
            "--config",
            "ui.log-word-wrap=false",
        ])
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::SpawnJj)?;
    if !out.status.success() {
        return Err(Error::Jj {
            command,
            status: out.status,
        });
    }
    String::from_utf8(out.stdout).map_err(|e| Error::JjOutput {
        command,
        output: String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use hegel::generators;

    use super::{find_workspace, parse_log, parse_log_line};
    use crate::error::Error;
    use crate::model::DiffStat;

    /// The output skips jj's virtual root commit and fails on the first line that does not parse,
    /// naming that line.
    #[test]
    fn jj_log_skips_the_root_and_rejects_bad_lines() {
        let id = "1".repeat(40);
        let root = "0".repeat(40);
        let parsed = parse_log(&format!("{id} 1 2 3\n{root}\n")).unwrap();
        let diff = DiffStat {
            insertions: 2,
            deletions: 3,
            files: 1,
        };
        let want = gix::ObjectId::from_hex(id.as_bytes()).unwrap();
        assert_eq!(parsed, [(want, Some(diff))]);

        let err = parse_log(&format!("{id}\nnot a commit\n")).unwrap_err();
        assert!(
            matches!(&err, Error::JjOutput { output, .. } if output == "not a commit"),
            "{err:?}"
        );
    }

    /// Every line `LOG_TEMPLATE` can print parses back to what it encodes: a bare commit ID, or
    /// an ID followed by files, insertions, and deletions.
    #[hegel::test]
    fn jj_log_lines_round_trip(tc: hegel::TestCase) {
        let id = gix::ObjectId::Sha1(tc.draw(generators::arrays(generators::integers::<u8>())));
        let diff = tc
            .draw(generators::optional(generators::arrays::<_, u64, 3>(
                generators::integers(),
            )))
            .map(|[files, insertions, deletions]| DiffStat {
                insertions,
                deletions,
                files,
            });
        let line = match diff {
            None => id.to_string(),
            Some(d) => format!("{id} {} {} {}", d.files, d.insertions, d.deletions),
        };
        assert_eq!(parse_log_line(&line), Some((id, diff)));
    }

    /// A line with a count field missing or extra is malformed, never a shifted or truncated
    /// numstat.
    #[hegel::test]
    fn jj_log_lines_reject_wrong_field_counts(tc: hegel::TestCase) {
        let id = gix::ObjectId::Sha1(tc.draw(generators::arrays(generators::integers::<u8>())));
        let counts = tc.draw(generators::vecs(generators::integers::<u64>()).max_size(8));
        tc.assume(!counts.is_empty() && counts.len() != 3);
        let line = std::iter::once(id.to_string())
            .chain(counts.iter().map(u64::to_string))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(parse_log_line(&line), None);
    }

    /// The workspace is the nearest directory holding `.jj`. When stopping at git, a nearer `.git`
    /// (directory or file) means there is none, while a directory holding both, as a colocated
    /// workspace does, is still jj.
    // Each case creates up to 15 directories and files. 50 cases cover the 6 distinct marker sets
    // per level while keeping the test under a second.
    #[hegel::test(test_cases = 50)]
    fn jj_workspace_is_the_nearest_jj_directory(tc: hegel::TestCase) {
        // One marker set per nested directory, outermost first. Bit 0 adds a `.jj` directory, bit 1
        // a `.git`, and bit 2 makes that `.git` a file.
        let levels = tc.draw(
            generators::vecs(generators::integers::<u8>().max_value(7))
                .min_size(1)
                .max_size(5),
        );
        let stop_at_git = tc.draw(generators::booleans());

        let root = tempfile::tempdir().unwrap();
        let mut dir = root.path().to_path_buf();
        let mut dirs = Vec::new();
        for (i, &markers) in levels.iter().enumerate() {
            dir.push(format!("l{i}"));
            std::fs::create_dir(&dir).unwrap();
            if markers & 1 != 0 {
                std::fs::create_dir(dir.join(".jj")).unwrap();
            }
            if markers & 2 != 0 {
                if markers & 4 != 0 {
                    std::fs::write(dir.join(".git"), "gitdir: elsewhere\n").unwrap();
                } else {
                    std::fs::create_dir(dir.join(".git")).unwrap();
                }
            }
            dirs.push(dir.canonicalize().unwrap());
        }

        // `Some` when one of our directories decides the answer; `None` when the search must
        // continue above the temporary directory.
        let mut decided = None;
        for (markers, dir) in levels.iter().zip(&dirs).rev() {
            if markers & 1 != 0 {
                decided = Some(Some(dir.clone()));
                break;
            }
            if stop_at_git && markers & 2 != 0 {
                decided = Some(None);
                break;
            }
        }
        let found = find_workspace(&dir, stop_at_git);
        match decided {
            Some(expected) => assert_eq!(found, expected),
            // The host's own directories may hold a workspace; it just must not be one of ours.
            None => assert!(
                found.as_ref().is_none_or(|f| !f.starts_with(&dirs[0])),
                "found {found:?}"
            ),
        }
    }
}
