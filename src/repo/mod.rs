//! Infrastructure: the only module that reads repositories, through gix's object store, the `jj`
//! CLI, and the filesystem.
//!
//! Two adapters select the commits of a Revision range: `git` resolves a git revision range, and
//! `jj` evaluates a revset with the `jj` CLI. Each also opens the repository and supplies the
//! attributes for diffs. Either way, `commits` reads the selected commits from the git object store
//! and `diff` computes their numstats.

mod commits;
mod diff;
mod git;
mod jj;

use std::path::{Path, PathBuf};

use clap::ValueEnum;
use rayon::prelude::*;

use crate::error::{Error, Result};
use crate::model::{CommitMeta, Contributor, DiffStat};

/// The object cache each thread-local repository gets, so the trees and blobs a commit's walk and
/// diff read repeatedly come from memory instead of the pack. 8 MiB comfortably holds a commit's
/// trees.
const OBJECT_CACHE_BYTES: usize = 8 * 1024 * 1024;

/// Which version control system reads the repository and interprets the revision range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Vcs {
    /// Use jj in a jj workspace (colocated or not), git otherwise
    Auto,
    /// Read the git repository and take a git revision range
    Git,
    /// Read the jj workspace and take a jj revset
    Jj,
}

/// What [`Repo::read`] finds in a Revision range.
#[derive(Debug)]
pub struct Commits {
    /// The commits that `keep` accepted, each with its numstat, in walk order.
    pub kept: Vec<(CommitMeta, DiffStat)>,
    /// The Reviewers of every commit in the range, kept or not, one list per commit in walk order.
    /// `None` unless they were requested.
    pub reviewers: Option<Vec<Vec<Contributor>>>,
}

/// A commit as the walk reads it: its metadata, its Reviewers (empty unless requested), and how to
/// get its numstat.
struct WalkedCommit {
    meta: CommitMeta,
    reviewers: Vec<Contributor>,
    numstat: Numstat,
}

/// How to obtain a commit's numstat, decided once when the walk reads the commit.
#[derive(Clone, Copy)]
enum Numstat {
    /// gix diffs the tree of commit `id` against the tree of commit `base`. Without a `base`, it
    /// diffs against the empty tree: for a root commit, and for a commit at the shallow boundary,
    /// whose parents are missing from the object database.
    Diff {
        id: gix::ObjectId,
        base: Option<gix::ObjectId>,
    },
    /// The numstat is already known. A merge contributes nothing, matching `git log --numstat`, and
    /// jj computes the numstat where a conflict makes the git trees meaningless.
    Known(DiffStat),
}

/// The commits an adapter selects, in walk order, each with its numstat when the adapter already
/// knows it.
type Selected<'r> = Box<dyn Iterator<Item = Result<(gix::ObjectId, Option<DiffStat>)>> + 'r>;

/// The adapter that selects the commits of a Revision range.
enum Revisions {
    Git,
    /// The jj workspace rooted at this path.
    Jj(PathBuf),
}

impl Revisions {
    fn select<'r>(&self, repo: &'r gix::Repository, range: Option<&str>) -> Result<Selected<'r>> {
        match self {
            Self::Git => git::select(repo, range),
            Self::Jj(workspace) => jj::select(workspace, range),
        }
    }

    /// A diff resource cache with the `.gitattributes` of the checkout being read.
    fn diff_cache(&self, repo: &gix::Repository) -> Result<gix::diff::blob::Platform> {
        match self {
            Self::Git => git::diff_cache(repo),
            Self::Jj(_) => jj::diff_cache(repo),
        }
    }
}

/// A git repository or jj workspace.
pub struct Repo {
    repo: gix::ThreadSafeRepository,
    revisions: Revisions,
}

impl Repo {
    /// Open the repository containing `path`.
    ///
    /// For git, this honors `GIT_DIR` and related environment variables first (git sets them when
    /// running hooks and for `--git-dir` invocations), then searches upward from `path`. For jj, it
    /// opens the git repository that backs the nearest jj workspace. [`Vcs::Auto`] picks git when
    /// `GIT_DIR` is set, and otherwise whichever of the two is nearer, with jj for a colocated
    /// workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if no repository can be discovered, or if `jj` cannot run or does not
    /// report a git-backed repository.
    pub fn open(path: impl AsRef<Path>, vcs: Vcs) -> Result<Self> {
        let path = path.as_ref();
        let git_dir_set = std::env::var_os("GIT_DIR").is_some();
        Ok(match jj_workspace_to_read(path, vcs, git_dir_set)? {
            None => Self {
                repo: git::open(path)?,
                revisions: Revisions::Git,
            },
            Some(workspace) => Self {
                repo: jj::open(&workspace)?,
                revisions: Revisions::Jj(workspace),
            },
        })
    }

    /// Whether the repository is a shallow clone. Its truncated history makes every count differ
    /// from the full clone's, so callers may want to warn. A shallow-file path that cannot be
    /// represented natively counts as "not shallow", as the walk treats it.
    #[must_use]
    pub fn is_shallow(&self) -> bool {
        self.repo.to_thread_local().is_shallow().unwrap_or(false)
    }

    /// Read every commit in `range`: a git revision range, or a revset in a jj workspace. `None`
    /// means `HEAD` for git and `::@` for jj. Only the commits that `keep` accepts get a numstat,
    /// since diffing is the costly step. Merge commits contribute nothing, matching
    /// `git log --numstat`'s default, except at the shallow boundary, where a merge counts its
    /// whole tree.
    ///
    /// Reviewers are read only with `reviews`, since they feed only the reviews table and decoding
    /// every commit message is costly on large histories. They cover every commit in the range,
    /// whatever `keep` says.
    ///
    /// # Errors
    ///
    /// Returns an error if the range cannot be resolved, a commit object cannot be read, or a
    /// commit's tree or its parent's tree cannot be diffed.
    pub fn read(
        &self,
        range: Option<&str>,
        mut keep: impl FnMut(&CommitMeta) -> bool,
        reviews: bool,
    ) -> Result<Commits> {
        // The walk's repository and its object cache drop here, before each diff worker opens its
        // own.
        let (kept, reviewers) = {
            let mut repo = self.repo.to_thread_local();
            // The metadata read then reuses the commits that the traversal already decoded.
            repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
            let selected = self.revisions.select(&repo, range)?;
            let mut reviewers = reviews.then(Vec::new);
            let mut kept = Vec::new();
            for commit in commits::read(&repo, selected, reviews) {
                let commit = commit?;
                if let Some(all) = &mut reviewers {
                    all.push(commit.reviewers);
                }
                if keep(&commit.meta) {
                    kept.push((commit.meta, commit.numstat));
                }
            }
            (kept, reviewers)
        };
        let kept = kept
            .into_par_iter()
            .map_init(
                || diff::Worker::new(self.repo.to_thread_local(), &self.revisions),
                |worker, (meta, numstat)| Ok((meta, worker.numstat(numstat)?)),
            )
            .collect::<Result<_>>()?;
        Ok(Commits { kept, reviewers })
    }
}

/// The jj workspace to read for `path`, or `None` to read it as a git repository. This looks for
/// `.jj` and `.git` on disk.
///
/// [`Vcs::Auto`] picks git when `GIT_DIR` is set, as it is inside git hooks. Otherwise it picks the
/// nearer of a jj workspace and a git repository, with jj for a colocated workspace. [`Vcs::Jj`]
/// ignores `GIT_DIR` and any nearer git repository.
///
/// # Errors
///
/// Returns [`Error::NoJjWorkspace`] if [`Vcs::Jj`] finds no workspace.
fn jj_workspace_to_read(path: &Path, vcs: Vcs, git_dir_set: bool) -> Result<Option<PathBuf>> {
    match vcs {
        Vcs::Git => Ok(None),
        Vcs::Auto if git_dir_set => Ok(None),
        Vcs::Auto => Ok(jj::find_workspace(path, true)),
        Vcs::Jj => jj::find_workspace(path, false)
            .map(Some)
            .ok_or_else(|| Error::NoJjWorkspace {
                path: std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Vcs, jj_workspace_to_read};

    /// Every `--vcs` choice, with and without `GIT_DIR`, from a colocated workspace and from a git
    /// repository nested in it. `GIT_DIR` steers only `auto`, and forced jj ignores the nearer git
    /// repository.
    #[test]
    fn backend_choice_follows_vcs_and_git_dir() {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws");
        std::fs::create_dir_all(ws.join(".jj")).unwrap();
        std::fs::create_dir(ws.join(".git")).unwrap();
        let nested = ws.join("nested");
        std::fs::create_dir_all(nested.join(".git")).unwrap();
        let ws = ws.canonicalize().unwrap();

        let cases = [
            (&ws, Vcs::Auto, false, Some(&ws)),
            (&ws, Vcs::Auto, true, None),
            (&ws, Vcs::Git, false, None),
            (&ws, Vcs::Jj, true, Some(&ws)),
            (&nested, Vcs::Auto, false, None),
            (&nested, Vcs::Auto, true, None),
            (&nested, Vcs::Jj, false, Some(&ws)),
        ];
        for (path, vcs, git_dir_set, want) in cases {
            let got = jj_workspace_to_read(path, vcs, git_dir_set).unwrap();
            assert_eq!(
                got.as_ref(),
                want,
                "{path:?} {vcs:?} git_dir_set={git_dir_set}"
            );
        }
    }
}
