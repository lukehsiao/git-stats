//! The git adapter: gix discovers the repository and resolves a git revision range.

use std::path::Path;

use super::{WalkedCommit, commits};
use crate::error::{Error, Result};

/// Discover the git repository containing `path`, honoring `GIT_DIR` and related environment
/// variables first.
pub(super) fn open(path: &Path) -> Result<gix::ThreadSafeRepository> {
    gix::ThreadSafeRepository::discover_with_environment_overrides(path)
        .map_err(|e| Error::OpenRepository(Box::new(e)))
}

/// Read every commit in the git revision `range`, which defaults to `HEAD`.
pub(super) fn walk(
    repo: &gix::Repository,
    range: Option<&str>,
    need_reviewers: bool,
) -> Result<Vec<WalkedCommit>> {
    let range = range.unwrap_or("HEAD");
    let (tips, hidden) = resolve_range(repo, range)?;
    let walk = repo
        .rev_walk(tips)
        .with_hidden(hidden)
        .all()
        .map_err(|e| Error::WalkRange {
            range: range.to_string(),
            source: Box::new(e),
        })?;
    // Stream the IDs so each commit is read right after the traversal decoded it, while it is still
    // in the object cache.
    let commits = walk.map(|info| {
        info.map(|info| (info.id, None))
            .map_err(|e| Error::ReadCommit(Box::new(e)))
    });
    commits::read(repo, commits, need_reviewers)
}

/// A diff resource cache with the `.gitattributes` of the checkout, read from the index as gix
/// reads them.
pub(super) fn diff_cache(repo: &gix::Repository) -> Result<gix::diff::blob::Platform> {
    repo.diff_resource_cache_for_tree_diff()
        .map_err(|e| Error::DiffStats(Box::new(e)))
}

/// Tips to walk from, and commits to hide, for a revision range.
type RangeEnds = (Vec<gix::ObjectId>, Vec<gix::ObjectId>);

/// Resolve a revision range into walk tips and hidden commits. Each endpoint is fully git-spelled
/// (refs, short hashes, `@{n}`, ...); only the `..`/`...` operators are interpreted here. Exotic
/// gitrevisions(7) forms are unsupported.
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
    // git peels tags recursively at rev-list endpoints, so an annotated tag must resolve to its
    // target commit here. Without peeling, a tag OID on the hidden side matches nothing during the
    // walk and the range silently degrades to whole-repo history.
    Ok(repo
        .rev_parse_single(rev)
        .map_err(|e| err(Box::new(e)))?
        .object()
        .map_err(|e| err(Box::new(e)))?
        .peel_to_commit()
        .map_err(|e| err(Box::new(e)))?
        .id)
}

/// Every merge base of `a` and `b`, hidden from the walk to form the symmetric difference.
/// Criss-cross histories have several, and git hides them all; hiding only the "best" one leaks the
/// other bases' ancestries into the walk. Disjoint histories have none; then nothing is hidden and
/// the symmetric difference is simply everything reachable from either tip.
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
