//! Numstat computation: gix diffs each commit's tree against its base, in parallel.

use super::{Numstat, OBJECT_CACHE_BYTES, Revisions};
use crate::error::{Error, Result};
use crate::model::DiffStat;

/// Per-rayon-worker state: a thread-local repository and two diff resource caches built once and
/// reused for every commit this worker handles. `gix::Tree::stats` would instead build a fresh
/// cache per call, which re-parses the on-disk index and reassembles the attribute stack each time;
/// on a large history that rebuild, not the diff itself, dominates the runtime.
pub(super) struct Worker<'a> {
    repo: gix::Repository,
    revisions: &'a Revisions,
    // `for_each_to_obtain_tree_with_cache` holds one cache for the tree walk while each change's
    // line diff needs another, so a worker keeps two. They are built lazily because the cache
    // constructor is fallible and rayon's `map_init` initializer cannot return a `Result`.
    caches: Option<(gix::diff::blob::Platform, gix::diff::blob::Platform)>,
}

impl<'a> Worker<'a> {
    pub(super) fn new(mut repo: gix::Repository, revisions: &'a Revisions) -> Self {
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        Self {
            repo,
            revisions,
            caches: None,
        }
    }

    pub(super) fn numstat(&mut self, numstat: Numstat) -> Result<DiffStat> {
        let (id, base) = match numstat {
            Numstat::Diff { id, base } => (id, base),
            Numstat::Known(diff) => return Ok(diff),
        };
        if self.caches.is_none() {
            let walk = self.revisions.diff_cache(&self.repo)?;
            let count = walk.clone();
            self.caches = Some((walk, count));
        }
        let Self { repo, caches, .. } = self;
        let (walk_cache, count_cache) = caches.as_mut().expect("initialized above");
        diff_numstat(repo, walk_cache, count_cache, id, base)
    }
}

/// The numstat of commit `id` against commit `base`, or against the empty tree without a `base`.
fn diff_numstat(
    repo: &gix::Repository,
    walk_cache: &mut gix::diff::blob::Platform,
    count_cache: &mut gix::diff::blob::Platform,
    id: gix::ObjectId,
    base: Option<gix::ObjectId>,
) -> Result<DiffStat> {
    let tree_of = |id: gix::ObjectId| {
        repo.find_commit(id)
            .map_err(|e| Error::ReadCommit(Box::new(e)))?
            .tree()
            .map_err(|e| Error::ReadCommit(Box::new(e)))
    };
    let new_tree = tree_of(id)?;
    let old_tree = match base {
        Some(base) => tree_of(base)?,
        None => repo.empty_tree(),
    };

    // Equivalent to `gix::Tree::stats`, but driving caller-owned caches so the index and attribute
    // stack are read once per worker rather than per commit.
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
                    // A binary change has no line counts (numstat's `-  -  path`) but still counts
                    // as a changed file, as in `git diff --shortstat`.
                    Some(None) => files += 1,
                    None => {}
                }
            }
            // The resource cache only grows; clear it between changes to bound memory.
            count_cache.clear_resource_cache_keep_allocation();
            Ok(std::ops::ControlFlow::Continue(()))
        })
        .map_err(|e| Error::DiffStats(Box::new(e)))?;
    walk_cache.clear_resource_cache_keep_allocation();

    Ok(DiffStat {
        insertions,
        deletions,
        files,
    })
}

/// The numstat of a submodule (gitlink) change, which carries no blob to diff. git renders the
/// pointer as a one-line `Subproject commit <hash>` pseudo-file, so an added gitlink is +1, a
/// removed one -1, and a repointed one +1/-1. A renamed gitlink is paired into a single file whose
/// line only changes when the commit it points to also changed, matching git's `0 0 old => new`.
/// Returns `None` for changes not purely between gitlinks; a blob<->gitlink type change falls
/// through to the regular blob diff.
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
