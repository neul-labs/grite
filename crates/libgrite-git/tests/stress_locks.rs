//! Stress tests for lock contention
//!
//! These tests verify that lock operations handle concurrent access correctly.
//! Note: LockManager contains git2::Repository which isn't thread-safe,
//! so each thread opens its own LockManager instance.

use libgrite_git::LockManager;
use std::sync::{Arc, Barrier, atomic::{AtomicUsize, Ordering}};
use std::thread;
use tempfile::tempdir;

fn init_git_repo(path: &std::path::Path) {
    let repo = git2::Repository::init(path).expect("Failed to init git repo");

    // Create initial commit
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .expect("Failed to create initial commit");
}

#[test]
fn test_concurrent_lock_acquisition_same_resource() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let git_dir = Arc::new(git_dir);
    let resource = "issue:test123";
    let num_threads = 8;
    let barrier = Arc::new(Barrier::new(num_threads));
    let success_counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let git_dir = Arc::clone(&git_dir);
            let barrier = Arc::clone(&barrier);
            let success_counter = Arc::clone(&success_counter);
            let actor = format!("actor-{:02}", thread_id);

            thread::spawn(move || {
                let lock_manager = LockManager::open(&git_dir).expect("Failed to open lock manager");
                barrier.wait();

                // Try to acquire lock
                match lock_manager.acquire(resource, &actor, None) {
                    Ok(_lock) => {
                        // Got the lock, hold it briefly
                        thread::sleep(std::time::Duration::from_millis(10));
                        let _ = lock_manager.release(resource, &actor);
                        success_counter.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => {}
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let successes = success_counter.load(Ordering::SeqCst);

    // At least one thread should have gotten the lock
    assert!(
        successes >= 1,
        "Expected at least 1 success, got {}",
        successes
    );
    println!("Lock same-resource test: {} threads got the lock", successes);
}

#[test]
fn test_concurrent_lock_acquisition_different_resources() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let git_dir = Arc::new(git_dir);
    let num_threads = 8;
    let barrier = Arc::new(Barrier::new(num_threads));
    let success_counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let git_dir = Arc::clone(&git_dir);
            let barrier = Arc::clone(&barrier);
            let success_counter = Arc::clone(&success_counter);
            let actor = format!("actor-{:02}", thread_id);
            let resource = format!("issue:test{:02}", thread_id);

            thread::spawn(move || {
                let lock_manager = LockManager::open(&git_dir).expect("Failed to open lock manager");
                barrier.wait();

                // Try to acquire lock on unique resource
                match lock_manager.acquire(&resource, &actor, None) {
                    Ok(_lock) => {
                        thread::sleep(std::time::Duration::from_millis(5));
                        let _ = lock_manager.release(&resource, &actor);
                        success_counter.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => {}
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let successes = success_counter.load(Ordering::SeqCst);

    // Most threads should succeed since they're locking different resources
    assert!(
        successes >= num_threads / 2,
        "Expected at least {} successes, got {}",
        num_threads / 2,
        successes
    );
    println!(
        "Lock different-resources test: {}/{} succeeded",
        successes, num_threads
    );
}

#[test]
fn test_lock_acquire_release_cycle() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let git_dir = Arc::new(git_dir);
    let resource = "issue:shared";
    let num_threads = 4;
    let cycles_per_thread = 5; // Reduced for faster test
    let barrier = Arc::new(Barrier::new(num_threads));
    let total_acquired = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let git_dir = Arc::clone(&git_dir);
            let barrier = Arc::clone(&barrier);
            let total_acquired = Arc::clone(&total_acquired);
            let actor = format!("actor-{:02}", thread_id);

            thread::spawn(move || {
                let lock_manager = LockManager::open(&git_dir).expect("Failed to open lock manager");
                barrier.wait();

                for _ in 0..cycles_per_thread {
                    // Try to acquire
                    if lock_manager.acquire(resource, &actor, None).is_ok() {
                        total_acquired.fetch_add(1, Ordering::SeqCst);
                        // Hold briefly
                        thread::sleep(std::time::Duration::from_millis(5));
                        // Release
                        let _ = lock_manager.release(resource, &actor);
                    }
                    // Small delay before next attempt
                    thread::sleep(std::time::Duration::from_millis(2));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let acquired = total_acquired.load(Ordering::SeqCst);

    // Some acquisitions should succeed across all threads
    assert!(acquired > 0, "No lock acquisitions succeeded");
    println!(
        "Lock cycle test: {} total acquisitions across {} threads",
        acquired, num_threads
    );
}

#[test]
fn test_lock_list_during_operations() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let git_dir = Arc::new(git_dir);
    let barrier = Arc::new(Barrier::new(3));
    let acquire_success = Arc::new(AtomicUsize::new(0));
    let list_success = Arc::new(AtomicUsize::new(0));
    let gc_success = Arc::new(AtomicUsize::new(0));

    // Thread 1: Acquire and release locks
    let gd1 = Arc::clone(&git_dir);
    let b1 = Arc::clone(&barrier);
    let as1 = Arc::clone(&acquire_success);
    let acquire_handle = thread::spawn(move || {
        let lm = LockManager::open(&gd1).expect("Failed to open lock manager");
        b1.wait();

        for i in 0..10 {
            let resource = format!("issue:list-test-{}", i);
            let actor = "list-actor";
            if lm.acquire(&resource, actor, Some(100)).is_ok() {
                as1.fetch_add(1, Ordering::SeqCst);
                // Short TTL, don't release - let them expire
            }
        }
    });

    // Thread 2: List locks
    let gd2 = Arc::clone(&git_dir);
    let b2 = Arc::clone(&barrier);
    let ls2 = Arc::clone(&list_success);
    let list_handle = thread::spawn(move || {
        let lm = LockManager::open(&gd2).expect("Failed to open lock manager");
        b2.wait();

        for _ in 0..20 {
            if lm.list_locks().is_ok() {
                ls2.fetch_add(1, Ordering::SeqCst);
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
    });

    // Thread 3: Run GC
    let gd3 = Arc::clone(&git_dir);
    let b3 = Arc::clone(&barrier);
    let gs3 = Arc::clone(&gc_success);
    let gc_handle = thread::spawn(move || {
        let lm = LockManager::open(&gd3).expect("Failed to open lock manager");
        b3.wait();

        for _ in 0..5 {
            thread::sleep(std::time::Duration::from_millis(30));
            if lm.gc().is_ok() {
                gs3.fetch_add(1, Ordering::SeqCst);
            }
        }
    });

    acquire_handle.join().unwrap();
    list_handle.join().unwrap();
    gc_handle.join().unwrap();

    let acquires = acquire_success.load(Ordering::SeqCst);
    let lists = list_success.load(Ordering::SeqCst);
    let gcs = gc_success.load(Ordering::SeqCst);

    // All types of operations should succeed at least sometimes
    assert!(acquires > 0, "No locks acquired");
    assert!(lists > 0, "No list operations succeeded");
    assert!(gcs > 0, "No GC runs succeeded");

    println!(
        "Lock operations test: {} acquires, {} lists, {} GCs",
        acquires, lists, gcs
    );
}
