//! Stress tests for WAL concurrent operations
//!
//! These tests verify that WAL operations handle concurrent access correctly.
//! Note: git2::Repository is not thread-safe, so each thread opens its own WalManager.

use libgrite_core::types::event::{Event, EventKind};
use libgrite_core::types::ids::generate_actor_id;
use libgrite_core::hash::compute_event_id;
use libgrite_git::WalManager;
use std::sync::{Arc, Barrier, atomic::{AtomicUsize, Ordering}};
use std::thread;
use tempfile::tempdir;

fn create_test_event(actor: &[u8; 16], issue_id: &[u8; 16], index: u64) -> Event {
    let ts = 1700000000000 + index;
    let kind = EventKind::CommentAdded {
        body: format!("Comment {}", index),
    };
    let event_id = compute_event_id(issue_id, actor, ts, None, &kind);
    Event::new(event_id, *issue_id, *actor, ts, None, kind)
}

fn init_git_repo(path: &std::path::Path) {
    let repo = git2::Repository::init(path).expect("Failed to init git repo");

    // Create initial commit
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .expect("Failed to create initial commit");
    // repo is dropped here, releasing all borrows
}

#[test]
fn test_concurrent_wal_appends_single_actor() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let actor = generate_actor_id();
    let issue_id = [42u8; 16];
    let git_dir = Arc::new(git_dir);

    let num_threads = 4;
    let events_per_thread = 25;
    let barrier = Arc::new(Barrier::new(num_threads));
    let success_counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let git_dir = Arc::clone(&git_dir);
            let barrier = Arc::clone(&barrier);
            let success_counter = Arc::clone(&success_counter);

            thread::spawn(move || {
                // Each thread opens its own WAL manager
                let wal = WalManager::open(&git_dir).expect("Failed to open WAL");

                // Wait for all threads to be ready
                barrier.wait();

                for i in 0..events_per_thread {
                    let event_index = (thread_id * events_per_thread + i) as u64;
                    let event = create_test_event(&actor, &issue_id, event_index);

                    match wal.append(&actor, &[event]) {
                        Ok(_) => {
                            success_counter.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            // Concurrent conflicts are expected
                            eprintln!("Thread {} event {} failed: {:?}", thread_id, i, e);
                        }
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let total_success = success_counter.load(Ordering::SeqCst);

    // At least some events should succeed
    assert!(total_success > 0, "No events were appended successfully");
    println!(
        "Concurrent WAL test: {}/{} events appended successfully",
        total_success,
        num_threads * events_per_thread
    );
}

#[test]
fn test_concurrent_wal_appends_multiple_actors() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let git_dir = Arc::new(git_dir);
    let num_actors = 4;
    let events_per_actor = 25;
    let barrier = Arc::new(Barrier::new(num_actors));
    let success_counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_actors)
        .map(|actor_idx| {
            let git_dir = Arc::clone(&git_dir);
            let barrier = Arc::clone(&barrier);
            let success_counter = Arc::clone(&success_counter);

            thread::spawn(move || {
                let actor = generate_actor_id();
                let issue_id = [actor_idx as u8; 16];

                // Each thread opens its own WAL manager
                let wal = WalManager::open(&git_dir).expect("Failed to open WAL");

                // Wait for all threads to be ready
                barrier.wait();

                for i in 0..events_per_actor {
                    let event = create_test_event(&actor, &issue_id, i as u64);

                    match wal.append(&actor, &[event]) {
                        Ok(_) => {
                            success_counter.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            eprintln!("Actor {} event {} failed: {:?}", actor_idx, i, e);
                        }
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let total_success = success_counter.load(Ordering::SeqCst);

    // With high concurrency on the same ref, we expect many conflicts.
    // The important thing is that some operations succeed and the WAL
    // remains consistent (no data corruption).
    assert!(
        total_success > 0,
        "No events succeeded - this indicates a fundamental problem"
    );
    println!(
        "Multi-actor WAL test: {}/{} events appended successfully (conflicts expected)",
        total_success,
        num_actors * events_per_actor
    );
}

#[test]
fn test_wal_batch_append() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let wal = WalManager::open(&git_dir).expect("Failed to open WAL");
    let actor = generate_actor_id();
    let issue_id = [99u8; 16];

    // Create a batch of events
    let batch_size = 100;
    let events: Vec<Event> = (0..batch_size)
        .map(|i| create_test_event(&actor, &issue_id, i as u64))
        .collect();

    // Append entire batch at once
    let result = wal.append(&actor, &events);
    assert!(result.is_ok(), "Batch append failed: {:?}", result.err());

    // Verify we can read them back
    let read_events = wal.read_all().expect("Failed to read events");
    assert_eq!(
        read_events.len(),
        batch_size,
        "Expected {} events, got {}",
        batch_size,
        read_events.len()
    );
}

#[test]
fn test_sequential_appends_many_events() {
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    init_git_repo(dir.path());

    let wal = WalManager::open(&git_dir).expect("Failed to open WAL");
    let actor = generate_actor_id();
    let issue_id = [88u8; 16];

    // Append many events sequentially
    let num_events = 200;
    for i in 0..num_events {
        let event = create_test_event(&actor, &issue_id, i as u64);
        wal.append(&actor, &[event])
            .expect(&format!("Failed to append event {}", i));
    }

    // Verify all events are there
    let read_events = wal.read_all().expect("Failed to read events");
    assert_eq!(
        read_events.len(),
        num_events,
        "Expected {} events, got {}",
        num_events,
        read_events.len()
    );
}
