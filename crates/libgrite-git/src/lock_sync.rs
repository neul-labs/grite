//! Reconciling lock refs (`refs/grite/locks/*`) between a clone and a remote.
//!
//! The remote's lock refs are the shared truth that clones agree on. Every
//! change to a remote lock ref is a compare-and-swap on the tip this clone
//! inspected (see `SyncManager::push_refs`), so the rules below only have to
//! decide which side's lock should survive. Two invariants keep mutual
//! exclusion:
//!
//! - A remote lock that is live and owned by another actor is never replaced
//!   or deleted, unless the local lock was written on top of it. That only
//!   happens if the local `acquire`, `renew` or `release` saw it as expired or
//!   as its own.
//! - A lock this clone fetched is only pushed back as a fast-forward of what
//!   the remote still has. Only locks owned by one of this repository's own
//!   actors (which linked worktrees share) can be new to the remote; any
//!   other actor's lock came from the remote, so once the remote drops it
//!   (released or collected) it is dropped here too, never pushed back.

use std::collections::HashSet;

use git2::{Oid, Repository};
use libgrite_core::Lock;

use crate::lock_manager::read_lock_at;

/// Prefix of lock refs
pub const LOCK_REF_PREFIX: &str = "refs/grite/locks/";

/// A lock that could not be pushed because another actor holds it on the
/// remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockConflict {
    /// Locked resource
    pub resource: String,
    /// Actor holding the remote lock
    pub owner: String,
    /// Time until the remote lock expires
    pub expires_in_ms: u64,
}

impl std::fmt::Display for LockConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "lock on {} is held by {} on the remote (expires in {}s)",
            self.resource,
            self.owner,
            self.expires_in_ms / 1000
        )
    }
}

/// What to do with one lock ref
#[derive(Debug)]
pub(crate) struct LockPlan {
    /// The local ref's new target (`None` deletes it)
    pub local: Option<Oid>,
    /// The remote ref's new target, if it changes (`Some(None)` deletes it)
    pub remote: Option<Option<Oid>>,
    /// A local lock lost to a live remote lock of another actor
    pub conflict: Option<LockConflict>,
    /// The local ref is ahead and only the push may settle it (a pull must
    /// leave it alone, or a release would be lost before it is pushed)
    pub pending_push: bool,
}

fn live(lock: &Option<Lock>) -> bool {
    lock.as_ref().is_some_and(|l| !l.is_expired())
}

/// `a` contains `b` in its history (or is `b`)
fn contains(repo: &Repository, a: Oid, b: Oid) -> bool {
    a == b || repo.graph_descendant_of(a, b).unwrap_or(false)
}

/// Decide the fate of one lock ref, given its local and remote tips (the
/// remote's objects must be present locally).
///
/// `local_actors` are the actor IDs (hex) of this repository. With `pushing`
/// false nothing is written; a lock that would have to be re-parented onto
/// the remote is just kept as is.
pub(crate) fn plan(
    repo: &Repository,
    local: Option<Oid>,
    remote: Option<Oid>,
    local_actors: &HashSet<String>,
    pushing: bool,
) -> Result<LockPlan, git2::Error> {
    let l_lock = local.and_then(|oid| read_lock_at(repo, oid));
    let r_lock = remote.and_then(|oid| read_lock_at(repo, oid));

    let local_wins = match (local, remote) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(l), Some(r)) if contains(repo, l, r) => true,
        (Some(l), Some(r)) if contains(repo, r, l) => false,
        // Unrelated histories: a live local lock beats an expired remote one,
        // or a remote one of the same owner that expires sooner.
        (Some(_), Some(_)) => {
            live(&l_lock)
                && match (&l_lock, &r_lock) {
                    (Some(l), Some(r)) if live(&r_lock) => {
                        l.owner == r.owner && l.expires_unix_ms >= r.expires_unix_ms
                    }
                    _ => true,
                }
        }
    };

    let mut plan = LockPlan {
        local,
        remote: None,
        conflict: None,
        pending_push: false,
    };

    if !local_wins {
        let r = remote.expect("remote wins only if it exists");
        if live(&r_lock) {
            plan.local = Some(r);
            if let (Some(l), Some(r)) = (&l_lock, &r_lock) {
                if live(&l_lock) && l.owner != r.owner {
                    plan.conflict = Some(LockConflict {
                        resource: r.resource.clone(),
                        owner: r.owner.clone(),
                        expires_in_ms: r.time_remaining_ms(),
                    });
                }
            }
        } else {
            // Nobody holds it: forget it on both sides.
            plan.local = None;
            plan.remote = Some(None);
        }
        return Ok(plan);
    }

    let l = local.expect("local wins only if it exists");
    if !live(&l_lock) {
        // Expired or released here. Delete it everywhere.
        plan.local = None;
        if remote.is_some() {
            plan.remote = Some(None);
            plan.pending_push = remote != Some(l);
        }
        return Ok(plan);
    }

    // A live local lock.
    let ours = l_lock
        .as_ref()
        .is_some_and(|lock| local_actors.contains(&lock.owner));
    match remote {
        // Ours and not pushed yet.
        None if ours => plan.remote = Some(Some(l)),
        // Someone else's, fetched earlier; the remote has since dropped it.
        None => plan.local = None,
        Some(r) if r == l => {}
        Some(r) if contains(repo, l, r) => plan.remote = Some(Some(l)),
        Some(r) if ours && pushing => {
            // Re-parent onto the remote tip so the push fast-forwards.
            let lock = l_lock.as_ref().expect("live lock exists");
            let merged = crate::lock_manager::write_lock_commit(repo, lock, &[l, r])
                .map_err(|e| git2::Error::from_str(&e.to_string()))?;
            plan.local = Some(merged);
            plan.remote = Some(Some(merged));
        }
        Some(_) => {}
    }
    if plan.remote.is_some() {
        plan.pending_push = true;
    }
    Ok(plan)
}
