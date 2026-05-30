//! Infrastructure: the only module that talks to gix and the filesystem.
//!
//! [`Repo`] has two constructors. [`Repo::open`] reads a real repository;
//! [`Repo::create_null`] returns an in-memory stand-in driven by canned commits,
//! so application and logic code can be tested without touching git.

use rayon::prelude::*;

use crate::error::{Error, Result};
use crate::model::{Author, CommitMeta, DiffStat, Trailer};

/// A commit yielded by [`Repo::walk`]: its logic-facing metadata plus an opaque
/// handle the wrapper uses to compute the numstat later.
pub struct WalkedCommit {
    pub meta: CommitMeta,
    is_merge: bool,
    handle: Handle,
}

/// How to obtain a commit's numstat: from gix (real) or from a canned value (null).
enum Handle {
    Real(gix::ObjectId),
    Null(DiffStat),
}

/// A canned commit used to build a nulled [`Repo`] in tests.
#[derive(Debug, Clone)]
pub struct NulledCommit {
    pub meta: CommitMeta,
    pub diff: DiffStat,
    pub is_merge: bool,
}

enum Backend {
    // Boxed because a `ThreadSafeRepository` is far larger than the null variant.
    Real(Box<gix::ThreadSafeRepository>),
    Null(Vec<NulledCommit>),
}

/// A git repository, or a nulled stand-in for it.
pub struct Repo {
    backend: Backend,
}

impl Repo {
    /// Open the repository containing `path`, searching upward like git does.
    ///
    /// # Errors
    ///
    /// Returns an error if no git repository can be discovered from `path`.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let repo = gix::discover(path).map_err(|e| Error::OpenRepository(Box::new(e)))?;
        Ok(Self {
            backend: Backend::Real(Box::new(repo.into_sync())),
        })
    }

    /// Build a nulled repository whose [`walk`](Self::walk) returns the given
    /// commits verbatim. The revision range is ignored, since range resolution
    /// is real-git behavior exercised separately by integration tests.
    #[must_use]
    pub fn create_null(commits: Vec<NulledCommit>) -> Self {
        Self {
            backend: Backend::Null(commits),
        }
    }

    /// Read commit metadata for every commit in `range`.
    ///
    /// # Errors
    ///
    /// Returns an error if the range cannot be resolved or a commit object
    /// cannot be read.
    pub fn walk(&self, range: &str) -> Result<Vec<WalkedCommit>> {
        match &self.backend {
            Backend::Real(tsr) => walk_real(&tsr.to_thread_local(), range),
            Backend::Null(commits) => Ok(commits
                .iter()
                .map(|c| WalkedCommit {
                    meta: c.meta.clone(),
                    is_merge: c.is_merge,
                    handle: Handle::Null(c.diff),
                })
                .collect()),
        }
    }

    /// Compute the numstat for each given commit. Merge commits contribute
    /// nothing, matching `git log --numstat`'s default.
    ///
    /// # Errors
    ///
    /// Returns an error if a commit's tree or its parent's tree cannot be
    /// diffed.
    pub fn numstats(&self, commits: &[&WalkedCommit]) -> Result<Vec<DiffStat>> {
        match &self.backend {
            Backend::Real(tsr) => commits
                .par_iter()
                .map_init(
                    || {
                        let mut repo = tsr.to_thread_local();
                        // Per-thread object cache so repeated tree/blob reads during
                        // diffing hit memory instead of the pack. 8 MiB comfortably
                        // holds a commit's trees at negligible per-thread cost.
                        repo.object_cache_size_if_unset(8 * 1024 * 1024);
                        repo
                    },
                    |repo, c| diffstat(c, Some(repo)),
                )
                .collect(),
            Backend::Null(_) => commits.iter().map(|c| diffstat(c, None)).collect(),
        }
    }
}

/// A commit's numstat. Merge commits contribute nothing (matching
/// `git log --numstat`); canned (null) commits return their stored value, and
/// real commits are diffed against their first parent using `repo`.
fn diffstat(commit: &WalkedCommit, repo: Option<&gix::Repository>) -> Result<DiffStat> {
    if commit.is_merge {
        return Ok(DiffStat::default());
    }
    match (&commit.handle, repo) {
        (Handle::Null(diff), _) => Ok(*diff),
        (Handle::Real(id), Some(repo)) => numstat_real(repo, *id),
        // A real handle only ever comes from the real backend, which always
        // supplies a repo, so this arm is unreachable in practice.
        (Handle::Real(_), None) => Ok(DiffStat::default()),
    }
}

fn walk_real(repo: &gix::Repository, range: &str) -> Result<Vec<WalkedCommit>> {
    let (tips, hidden) = resolve_range(repo, range)?;
    let mailmap = repo.open_mailmap();
    let walk = repo
        .rev_walk(tips)
        .with_hidden(hidden)
        .all()
        .map_err(|e| Error::WalkRange {
            range: range.to_string(),
            source: Box::new(e),
        })?;
    let mut out = Vec::new();
    for info in walk {
        let info = info.map_err(|e| Error::ReadCommit(Box::new(e)))?;
        let commit = repo
            .find_commit(info.id)
            .map_err(|e| Error::ReadCommit(Box::new(e)))?;
        let is_merge = commit.parent_ids().take(2).count() > 1;
        out.push(WalkedCommit {
            meta: commit_meta(&commit, &mailmap)?,
            is_merge,
            handle: Handle::Real(info.id),
        });
    }
    Ok(out)
}

fn commit_meta(commit: &gix::Commit, mailmap: &gix::mailmap::Snapshot) -> Result<CommitMeta> {
    let author = mailmap.resolve(
        commit
            .author()
            .map_err(|e| Error::ReadCommit(Box::new(e)))?,
    );
    let time_seconds = commit
        .committer()
        .map_err(|e| Error::ReadCommit(Box::new(e)))?
        .seconds();
    Ok(CommitMeta {
        author: Author {
            name: author.name.to_string(),
            email: author.email.to_string(),
        },
        time_seconds,
        trailers: parse_trailers(commit)?,
    })
}

fn parse_trailers(commit: &gix::Commit) -> Result<Vec<Trailer>> {
    let message = commit
        .message()
        .map_err(|e| Error::ReadCommit(Box::new(e)))?;
    let Some(body) = message.body() else {
        return Ok(Vec::new());
    };
    Ok(body
        .trailers()
        .map(|t| Trailer {
            token: t.token.to_string(),
            value: t.value.to_string(),
        })
        .collect())
}

fn numstat_real(repo: &gix::Repository, id: gix::ObjectId) -> Result<DiffStat> {
    let commit = repo
        .find_commit(id)
        .map_err(|e| Error::ReadCommit(Box::new(e)))?;
    let new_tree = commit.tree().map_err(|e| Error::ReadCommit(Box::new(e)))?;
    let old_tree = match commit.parent_ids().next() {
        Some(parent) => repo
            .find_commit(parent.detach())
            .map_err(|e| Error::ReadCommit(Box::new(e)))?
            .tree()
            .map_err(|e| Error::ReadCommit(Box::new(e)))?,
        None => repo.empty_tree(),
    };
    let stats = old_tree
        .changes()
        .map_err(|e| Error::DiffStats(Box::new(e)))?
        .stats(&new_tree)
        .map_err(|e| Error::DiffStats(Box::new(e)))?;
    Ok(DiffStat {
        insertions: stats.lines_added,
        deletions: stats.lines_removed,
        files: stats.files_changed,
    })
}

/// Tips to walk from, and commits to hide, for a revision range.
type RangeEnds = (Vec<gix::ObjectId>, Vec<gix::ObjectId>);

/// Resolve a revision range into walk tips and hidden commits. Each endpoint is
/// fully git-spelled (refs, short hashes, `@{n}`, ...); only the `..`/`...`
/// operators are interpreted here. Exotic gitrevisions(7) forms are unsupported.
fn resolve_range(repo: &gix::Repository, range: &str) -> Result<RangeEnds> {
    if let Some((a, b)) = range.split_once("...") {
        let a = single(repo, default_head(a))?;
        let b = single(repo, default_head(b))?;
        return Ok((vec![a, b], merge_bases(repo, a, b)));
    }
    if let Some((a, b)) = range.split_once("..") {
        let excluded = single(repo, default_head(a))?;
        let included = single(repo, default_head(b))?;
        return Ok((vec![included], vec![excluded]));
    }
    Ok((vec![single(repo, range)?], Vec::new()))
}

fn default_head(rev: &str) -> &str {
    if rev.is_empty() { "HEAD" } else { rev }
}

fn single(repo: &gix::Repository, rev: &str) -> Result<gix::ObjectId> {
    Ok(repo
        .rev_parse_single(rev)
        .map_err(|e| Error::ResolveRevision {
            revision: rev.to_string(),
            source: Box::new(e),
        })?
        .detach())
}

fn merge_bases(repo: &gix::Repository, a: gix::ObjectId, b: gix::ObjectId) -> Vec<gix::ObjectId> {
    // Disjoint histories have no merge base; then nothing is hidden and the
    // symmetric difference is simply everything reachable from either tip.
    match repo.merge_base(a, b) {
        Ok(base) => vec![base.detach()],
        Err(_) => Vec::new(),
    }
}

/// Parse a `--since`/`--until` style date into seconds since the Unix epoch.
/// Accepts ISO 8601, RFC 2822, unix timestamps, and relative spellings like
/// "2 weeks ago" (a documented subset of git's approxidate).
///
/// # Errors
///
/// Returns an error if the input is not a recognized date format.
pub fn parse_date(input: Option<&str>) -> Result<Option<i64>> {
    let Some(s) = input else { return Ok(None) };
    let now = std::time::SystemTime::now();
    let time = gix::date::parse(s, Some(now)).map_err(|e| Error::InvalidDate {
        input: s.to_string(),
        message: e.to_string(),
    })?;
    Ok(Some(time.seconds))
}

#[cfg(test)]
mod tests {
    use super::parse_date;

    #[test]
    fn parse_date_returns_none_for_no_input() {
        assert_eq!(parse_date(None).unwrap(), None);
    }

    #[test]
    fn parse_date_accepts_an_iso_date() {
        assert!(parse_date(Some("2020-01-01")).unwrap().is_some());
    }

    #[test]
    fn parse_date_rejects_unparseable_input() {
        assert!(parse_date(Some("not-a-real-date")).is_err());
    }
}
