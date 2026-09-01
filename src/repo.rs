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
    /// Open the repository containing `path`. Like git, this honors `GIT_DIR`
    /// and related environment variables first (git sets them when running
    /// hooks and for `--git-dir` invocations), then searches upward from
    /// `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if no git repository can be discovered from the
    /// environment or `path`.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let repo = gix::ThreadSafeRepository::discover_with_environment_overrides(path)
            .map_err(|e| Error::OpenRepository(Box::new(e)))?;
        Ok(Self {
            backend: Backend::Real(Box::new(repo)),
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

    /// Whether the repository is a shallow clone. Its truncated history makes
    /// every count differ from the full clone's, so callers may want to warn.
    /// Nulled repositories never are.
    #[must_use]
    pub fn is_shallow(&self) -> bool {
        match &self.backend {
            Backend::Real(tsr) => tsr.to_thread_local().is_shallow(),
            Backend::Null(_) => false,
        }
    }

    /// Read commit metadata for every commit in `range`. Trailers are parsed
    /// only when `need_trailers` is set, since they feed only the reviews table
    /// and decoding every commit message is costly on large histories.
    ///
    /// # Errors
    ///
    /// Returns an error if the range cannot be resolved or a commit object
    /// cannot be read.
    pub fn walk(&self, range: &str, need_trailers: bool) -> Result<Vec<WalkedCommit>> {
        match &self.backend {
            Backend::Real(tsr) => {
                let mut repo = tsr.to_thread_local();
                // Match `numstats`: an object cache lets the metadata read reuse
                // commits the traversal already decoded instead of re-reading packs.
                repo.object_cache_size_if_unset(8 * 1024 * 1024);
                walk_real(&repo, range, need_trailers)
            }
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
                    || Worker::new(tsr.to_thread_local()),
                    |worker, c| worker.numstat(c),
                )
                .collect(),
            Backend::Null(_) => Ok(commits.iter().map(|c| null_diffstat(c)).collect()),
        }
    }
}

/// Per-rayon-worker state for the real backend: a thread-local repository and
/// two diff resource caches built once and reused for every commit this worker
/// handles. `gix::Tree::stats` would instead build a fresh cache per call, which
/// re-parses the on-disk index and reassembles the attribute stack each time; on
/// a large history that rebuild, not the diff itself, dominates the runtime.
struct Worker {
    repo: gix::Repository,
    // Commits at the shallow boundary, whose parents exist in their headers
    // but not in the object database. `None` when the clone is not shallow.
    shallow: Option<gix::shallow::Commits>,
    // `for_each_to_obtain_tree_with_cache` holds one cache for the tree walk
    // while each change's line diff needs another, so a worker keeps two. They
    // are built lazily because the cache constructor is fallible and rayon's
    // `map_init` initializer cannot return a `Result`.
    caches: Option<(gix::diff::blob::Platform, gix::diff::blob::Platform)>,
}

impl Worker {
    fn new(mut repo: gix::Repository) -> Self {
        // Per-thread object cache so repeated tree/blob reads during diffing hit
        // memory instead of the pack. 8 MiB comfortably holds a commit's trees.
        repo.object_cache_size_if_unset(8 * 1024 * 1024);
        // An unreadable shallow file degrades to "not shallow"; a boundary
        // commit then surfaces its missing parent as a read error below.
        let shallow = repo.shallow_commits().ok().flatten();
        Self {
            repo,
            shallow,
            caches: None,
        }
    }

    fn numstat(&mut self, commit: &WalkedCommit) -> Result<DiffStat> {
        if commit.is_merge {
            return Ok(DiffStat::default());
        }
        let id = match &commit.handle {
            Handle::Real(id) => *id,
            // A real backend never yields a null handle; nothing to diff.
            Handle::Null(_) => return Ok(DiffStat::default()),
        };
        if self.caches.is_none() {
            let walk = self
                .repo
                .diff_resource_cache_for_tree_diff()
                .map_err(|e| Error::DiffStats(Box::new(e)))?;
            let count = self
                .repo
                .diff_resource_cache_for_tree_diff()
                .map_err(|e| Error::DiffStats(Box::new(e)))?;
            self.caches = Some((walk, count));
        }
        let Self {
            repo,
            shallow,
            caches,
        } = self;
        let (walk_cache, count_cache) = caches.as_mut().expect("initialized above");
        numstat_real(repo, walk_cache, count_cache, shallow.as_ref(), id)
    }
}

/// A canned (null-backend) commit's numstat: its stored value, or nothing for a
/// merge. Real handles never reach the null backend.
fn null_diffstat(commit: &WalkedCommit) -> DiffStat {
    if commit.is_merge {
        return DiffStat::default();
    }
    match &commit.handle {
        Handle::Null(diff) => *diff,
        Handle::Real(_) => DiffStat::default(),
    }
}

fn walk_real(
    repo: &gix::Repository,
    range: &str,
    need_trailers: bool,
) -> Result<Vec<WalkedCommit>> {
    let (tips, hidden) = resolve_range(repo, range)?;
    let mailmap = repo.open_mailmap();
    // git grafts commits at the shallow boundary as parentless, so one whose
    // header names several parents is not a merge there. An unreadable shallow
    // file degrades to "not shallow", mirroring Worker::new.
    let shallow = repo.shallow_commits().ok().flatten();
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
        // Decode the header once; the author, committer, parents, and message
        // accessors on `gix::Commit` would each rescan the raw bytes.
        let commit = commit.decode().map_err(|e| Error::DecodeCommit {
            id: info.id.to_string(),
            source: Box::new(e),
        })?;
        let is_boundary = shallow
            .as_ref()
            .is_some_and(|s| s.binary_search(&info.id).is_ok());
        let is_merge = !is_boundary && commit.parents.len() > 1;
        out.push(WalkedCommit {
            meta: commit_meta(&commit, &mailmap, need_trailers)?,
            is_merge,
            handle: Handle::Real(info.id),
        });
    }
    Ok(out)
}

fn commit_meta(
    commit: &gix::objs::CommitRef<'_>,
    mailmap: &gix::mailmap::Snapshot,
    need_trailers: bool,
) -> Result<CommitMeta> {
    let author = mailmap.resolve(
        commit
            .author()
            .map_err(|e| Error::ReadCommit(Box::new(e)))?,
    );
    let time_seconds = commit
        .committer()
        .map_err(|e| Error::ReadCommit(Box::new(e)))?
        .seconds();
    let trailers = if need_trailers {
        parse_trailers(commit)
    } else {
        Vec::new()
    };
    Ok(CommitMeta {
        author: Author {
            name: author.name.to_string(),
            email: author.email.to_string(),
        },
        time_seconds,
        trailers,
    })
}

fn parse_trailers(commit: &gix::objs::CommitRef<'_>) -> Vec<Trailer> {
    let Some(body) = commit.message().body() else {
        return Vec::new();
    };
    body.trailers()
        .map(|t| Trailer {
            token: t.token.to_string(),
            value: t.value.to_string(),
        })
        .collect()
}

fn numstat_real(
    repo: &gix::Repository,
    walk_cache: &mut gix::diff::blob::Platform,
    count_cache: &mut gix::diff::blob::Platform,
    shallow: Option<&gix::shallow::Commits>,
    id: gix::ObjectId,
) -> Result<DiffStat> {
    let commit = repo
        .find_commit(id)
        .map_err(|e| Error::ReadCommit(Box::new(e)))?;
    let new_tree = commit.tree().map_err(|e| Error::ReadCommit(Box::new(e)))?;
    // A commit at the shallow boundary names parents that are not in the
    // object database. git diffs it against the empty tree, exactly like a
    // root commit, so do the same instead of failing on the missing parent.
    let is_boundary = shallow.is_some_and(|s| s.binary_search(&id).is_ok());
    let parent = if is_boundary {
        None
    } else {
        commit.parent_ids().next()
    };
    let old_tree = match parent {
        Some(parent) => repo
            .find_commit(parent.detach())
            .map_err(|e| Error::ReadCommit(Box::new(e)))?
            .tree()
            .map_err(|e| Error::ReadCommit(Box::new(e)))?,
        None => repo.empty_tree(),
    };

    // Equivalent to `gix::Tree::stats`, but driving caller-owned caches so the
    // index and attribute stack are read once per worker rather than per commit.
    let (mut files, mut insertions, mut deletions) = (0u64, 0u64, 0u64);
    old_tree
        .changes()
        .map_err(|e| Error::DiffStats(Box::new(e)))?
        .for_each_to_obtain_tree_with_cache(&new_tree, walk_cache, |change| {
            if let Some((ins, del)) = gitlink_lines(&change) {
                files += 1;
                insertions += ins;
                deletions += del;
            } else {
                match change
                    .diff(count_cache)
                    .ok()
                    .and_then(|mut platform| platform.line_counts().ok())
                {
                    Some(Some(counts)) => {
                        files += 1;
                        insertions += u64::from(counts.insertions);
                        deletions += u64::from(counts.removals);
                    }
                    // A binary change has no line counts (numstat's `-  -  path`)
                    // but still counts as a changed file, as in `git diff --shortstat`.
                    Some(None) => files += 1,
                    None => {}
                }
            }
            // The resource cache only grows; clear it between changes to bound memory.
            count_cache.clear_resource_cache_keep_allocation();
            Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
        })
        .map_err(|e| Error::DiffStats(Box::new(e)))?;
    walk_cache.clear_resource_cache_keep_allocation();

    Ok(DiffStat {
        insertions,
        deletions,
        files,
    })
}

/// The numstat of a submodule (gitlink) change, which carries no blob to diff.
/// git renders the pointer as a one-line `Subproject commit <hash>` pseudo-file,
/// so an added gitlink is +1, a removed one -1, and a repointed one +1/-1. A
/// renamed gitlink is paired into a single file whose line only changes when the
/// commit it points to also changed, matching git's `0 0 old => new`.
/// Returns `None` for changes not purely between gitlinks; a blob<->gitlink
/// type change falls through to the regular blob diff.
fn gitlink_lines(change: &gix::object::tree::diff::Change<'_, '_, '_>) -> Option<(u64, u64)> {
    use gix::object::tree::diff::Change;
    match *change {
        Change::Addition { entry_mode, .. } if entry_mode.is_commit() => Some((1, 0)),
        Change::Deletion { entry_mode, .. } if entry_mode.is_commit() => Some((0, 1)),
        Change::Modification {
            previous_entry_mode,
            entry_mode,
            ..
        } if previous_entry_mode.is_commit() && entry_mode.is_commit() => Some((1, 1)),
        Change::Rewrite {
            source_entry_mode,
            source_id,
            entry_mode,
            id,
            ..
        } if source_entry_mode.is_commit() && entry_mode.is_commit() => {
            if source_id.detach() == id.detach() {
                Some((0, 0))
            } else {
                Some((1, 1))
            }
        }
        _ => None,
    }
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
        let hidden = merge_bases(repo, a, b, range)?;
        return Ok((vec![a, b], hidden));
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
    let err = |source: Box<dyn std::error::Error + Send + Sync>| Error::ResolveRevision {
        revision: rev.to_string(),
        source,
    };
    // git peels tags recursively at rev-list endpoints, so an annotated tag
    // must resolve to its target commit here. Without peeling, a tag OID on
    // the hidden side matches nothing during the walk and the range silently
    // degrades to whole-repo history.
    Ok(repo
        .rev_parse_single(rev)
        .map_err(|e| err(Box::new(e)))?
        .object()
        .map_err(|e| err(Box::new(e)))?
        .peel_to_commit()
        .map_err(|e| err(Box::new(e)))?
        .id)
}

/// Every merge base of `a` and `b`, hidden from the walk to form the symmetric
/// difference. Criss-cross histories have several, and git hides them all;
/// hiding only the "best" one leaks the other bases' ancestries into the walk.
/// Disjoint histories have none; then nothing is hidden and the symmetric
/// difference is simply everything reachable from either tip.
fn merge_bases(
    repo: &gix::Repository,
    a: gix::ObjectId,
    b: gix::ObjectId,
    range: &str,
) -> Result<Vec<gix::ObjectId>> {
    let bases = repo
        .merge_bases_many(a, &[b])
        .map_err(|e| Error::WalkRange {
            range: range.to_string(),
            source: Box::new(e),
        })?;
    Ok(bases.into_iter().map(gix::Id::detach).collect())
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
    let now = gix::date::Zoned::now();
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
