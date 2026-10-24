//! Numstat computation: gix diffs each commit's tree against its base, in parallel.

use std::ops::ControlFlow;

use gix::bstr::BStr;
use gix::diff::blob::ResourceKind;
use gix::diff::blob::platform::prepare_diff::Operation;
use gix::object::tree::EntryKind;
use gix::object::tree::diff::Change;

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
            let mut count = walk.clone();
            // numstat counts lines with the internal diff, whatever diff driver the attributes name.
            count.options.skip_internal_diff_if_external_is_configured = false;
            self.caches = Some((walk, count));
        }
        let Self { repo, caches, .. } = self;
        let (walk_cache, count_cache) = caches.as_mut().expect("initialized above");
        diff_numstat(repo, walk_cache, count_cache, id, base).map_err(|e| in_partial_clone(repo, e))
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
    let mut failure = None;
    let walked = old_tree
        .changes()
        .map_err(|e| Error::DiffStats(Box::new(e)))?
        .for_each_to_obtain_tree_with_cache(&new_tree, walk_cache, |change| {
            let lines = change_lines(repo, count_cache, id, &change);
            // The cache keys its contents by object ID alone, so clear it between changes. Else a
            // file would reuse the binary decision of an earlier file with the same contents at a
            // path with other attributes. Clearing also bounds memory.
            count_cache.clear_resource_cache_keep_allocation();
            match lines {
                Ok(Some((ins, del))) => {
                    files += 1;
                    insertions += ins;
                    deletions += del;
                }
                Ok(None) => {}
                Err(e) => {
                    failure = Some(e);
                    return Ok(ControlFlow::Break(()));
                }
            }
            Ok(ControlFlow::Continue(()))
        });
    walk_cache.clear_resource_cache_keep_allocation();
    // Stopping the walk early can make gix report a cancellation, so the cause comes first.
    if let Some(e) = failure {
        return Err(e);
    }
    walked.map_err(|e| Error::DiffStats(Box::new(e)))?;

    Ok(DiffStat {
        insertions,
        deletions,
        files,
    })
}

/// `e`, wrapped in [`Error::PartialClone`] when `repo` is a partial clone, where a missing object
/// is the likely cause of a failed read.
fn in_partial_clone(repo: &gix::Repository, e: Error) -> Error {
    let config = repo.config_snapshot();
    // git marks the remote of a partial clone with `promisor`. Older git versions set
    // `extensions.partialClone` instead.
    let promisor = config
        .plumbing()
        .sections_by_name("remote")
        .into_iter()
        .flatten()
        .filter_map(|remote| remote.value_implicit("promisor"))
        // git reads a key without a value as true.
        .any(|value| {
            value.is_none_or(|v| gix::config::Boolean::try_from(BStr::new(&v)).is_ok_and(|b| b.0))
        });
    if promisor || config.string("extensions.partialClone").is_some() {
        Error::PartialClone(Box::new(e))
    } else {
        e
    }
}

/// One side of a tree change: the entry's kind, object, and path, or `None` where no entry exists.
type Side<'a> = Option<(EntryKind, gix::ObjectId, &'a BStr)>;

/// The lines that one tree change inserts and deletes, as `git log --numstat` counts them, or
/// `None` when the change is not a file. A directory is not a file, because each file in it is a
/// change of its own. A binary file counts with no lines.
///
/// git shows a submodule (gitlink) as a one-line `Subproject commit <hash>` pseudo-file. So a
/// gitlink side adds or removes that one line, unless both sides point at the same commit, as in a
/// renamed submodule. A file swapped with a submodule counts as one file, with the file's lines on
/// its side and the pseudo-file's line on the other.
///
/// # Errors
///
/// Returns [`Error::ReadBlob`] if a file's contents cannot be read, for example because a partial
/// clone has not fetched them.
fn change_lines(
    repo: &gix::Repository,
    cache: &mut gix::diff::blob::Platform,
    commit: gix::ObjectId,
    change: &Change<'_, '_, '_>,
) -> Result<Option<(u64, u64)>> {
    let (old, new) = sides(change);
    if [old, new]
        .iter()
        .flatten()
        .any(|&(kind, ..)| kind == EntryKind::Tree)
    {
        return Ok(None);
    }
    let gitlink = |side: Side<'_>| match side {
        Some((EntryKind::Commit, id, _)) => Some(id),
        _ => None,
    };
    let (old_link, new_link) = (gitlink(old), gitlink(new));
    let same_pointer = old_link.is_some() && old_link == new_link;
    let mut insertions = u64::from(new_link.is_some() && !same_pointer);
    let mut deletions = u64::from(old_link.is_some() && !same_pointer);
    let old_file = old.filter(|&(kind, ..)| kind != EntryKind::Commit);
    let new_file = new.filter(|&(kind, ..)| kind != EntryKind::Commit);
    if old_file.is_some() || new_file.is_some() {
        match file_lines(repo, cache, commit, old_file, new_file)? {
            Some((ins, del)) => {
                insertions += ins;
                deletions += del;
            }
            // numstat shows a binary file as `-  -  path`, and `git diff --shortstat` still counts
            // it as a changed file.
            None => return Ok(Some((0, 0))),
        }
    }
    Ok(Some((insertions, deletions)))
}

/// The old and new sides of `change`.
fn sides<'a>(change: &Change<'a, '_, '_>) -> (Side<'a>, Side<'a>) {
    match *change {
        Change::Addition {
            location,
            entry_mode,
            id,
            ..
        } => (None, Some((entry_mode.kind(), id.detach(), location))),
        Change::Deletion {
            location,
            entry_mode,
            id,
            ..
        } => (Some((entry_mode.kind(), id.detach(), location)), None),
        Change::Modification {
            location,
            previous_entry_mode,
            previous_id,
            entry_mode,
            id,
        } => (
            Some((previous_entry_mode.kind(), previous_id.detach(), location)),
            Some((entry_mode.kind(), id.detach(), location)),
        ),
        Change::Rewrite {
            source_location,
            source_entry_mode,
            source_id,
            entry_mode,
            id,
            location,
            ..
        } => (
            Some((
                source_entry_mode.kind(),
                source_id.detach(),
                source_location,
            )),
            Some((entry_mode.kind(), id.detach(), location)),
        ),
    }
}

/// The lines that turning the `old` file into the `new` one inserts and deletes, where a missing
/// side is empty, or `None` when either file is binary. At least one side must be a file.
fn file_lines(
    repo: &gix::Repository,
    cache: &mut gix::diff::blob::Platform,
    commit: gix::ObjectId,
    old: Side<'_>,
    new: Side<'_>,
) -> Result<Option<(u64, u64)>> {
    let Some((kind, id, path)) = new.or(old) else {
        unreachable!("the caller passes at least one file");
    };
    // gix reads a null ID as an empty file, which stands in for a missing side: in an addition, a
    // deletion, or opposite a submodule.
    let empty = (kind, id.kind().null(), path);
    for (side, resource) in [
        (old, ResourceKind::OldOrSource),
        (new, ResourceKind::NewOrDestination),
    ] {
        let (kind, id, path) = side.unwrap_or(empty);
        let read_blob = |source| Error::ReadBlob {
            commit: commit.to_string(),
            path: path.to_string(),
            source,
        };
        // A tree entry with a null ID is corrupt. git fails to read it, but gix would read it as an
        // empty file.
        if side.is_some() && id.is_null() {
            return Err(read_blob("the tree entry has a null object ID".into()));
        }
        cache
            .set_resource(id, kind, path, resource, &repo.objects)
            .map_err(|e| read_blob(e.into()))?;
    }
    let prep = cache.prepare_diff().expect("both sides are set above");
    match prep.operation {
        Operation::InternalDiff { algorithm } => {
            // gix's own `interned_input` strips line endings, so `a` and `a\n` would compare
            // equal. git compares lines with their endings, so the diff keeps them.
            let input = gix::diff::blob::InternedInput::new(
                prep.old.intern_source(),
                prep.new.intern_source(),
            );
            let diff = gix::diff::blob::Diff::compute(algorithm, &input);
            Ok(Some((
                u64::from(diff.count_additions()),
                u64::from(diff.count_removals()),
            )))
        }
        Operation::SourceOrDestinationIsBinary => Ok(None),
        Operation::ExternalCommand { .. } => {
            unreachable!("the count cache turns external diffs off")
        }
    }
}
