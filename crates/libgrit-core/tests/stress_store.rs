//! Stress tests for store concurrent operations
//!
//! These tests verify that store operations handle concurrent access correctly.

use libgrit_core::store::GritStore;
use libgrit_core::types::event::{Event, EventKind};
use libgrit_core::types::ids::{generate_actor_id, generate_issue_id};
use libgrit_core::hash::compute_event_id;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::tempdir;

fn create_issue_event(actor: &[u8; 16], issue_id: &[u8; 16], index: u64) -> Event {
    let ts = 1700000000000 + index;
    let kind = EventKind::IssueCreated {
        title: format!("Issue {}", index),
        body: format!("Body for issue {}", index),
        labels: vec![],
    };
    let event_id = compute_event_id(issue_id, actor, ts, None, &kind);
    Event::new(event_id, *issue_id, *actor, ts, None, kind)
}

fn create_comment_event(actor: &[u8; 16], issue_id: &[u8; 16], ts_offset: u64) -> Event {
    let ts = 1700000000000 + ts_offset;
    let kind = EventKind::CommentAdded {
        body: format!("Comment at ts {}", ts),
    };
    let event_id = compute_event_id(issue_id, actor, ts, None, &kind);
    Event::new(event_id, *issue_id, *actor, ts, None, kind)
}

#[test]
fn test_concurrent_issue_creation() {
    let dir = tempdir().unwrap();
    let store = Arc::new(GritStore::open(dir.path()).expect("Failed to open store"));

    let num_threads = 8;
    let issues_per_thread = 50;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                let actor = generate_actor_id();

                // Wait for all threads to be ready
                barrier.wait();

                let mut success_count = 0;
                for i in 0..issues_per_thread {
                    let issue_id = generate_issue_id();
                    let event_index = (thread_id * issues_per_thread + i) as u64;
                    let event = create_issue_event(&actor, &issue_id, event_index);

                    match store.insert_event(&event) {
                        Ok(()) => success_count += 1,
                        Err(e) => {
                            eprintln!("Thread {} issue {} failed: {:?}", thread_id, i, e);
                        }
                    }
                }
                success_count
            })
        })
        .collect();

    let total_success: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let expected = num_threads * issues_per_thread;

    // All operations should succeed (sled handles concurrency)
    assert_eq!(
        total_success, expected,
        "Expected {} successes, got {}",
        expected, total_success
    );

    // Verify final count
    let stats = store.stats(dir.path()).expect("Failed to get stats");
    assert_eq!(
        stats.issue_count, expected,
        "Expected {} issues, got {}",
        expected, stats.issue_count
    );
}

#[test]
fn test_concurrent_comments_single_issue() {
    let dir = tempdir().unwrap();
    let store = Arc::new(GritStore::open(dir.path()).expect("Failed to open store"));

    let actor = generate_actor_id();
    let issue_id = generate_issue_id();

    // First create the issue
    let create_event = create_issue_event(&actor, &issue_id, 0);
    store.insert_event(&create_event).expect("Failed to create issue");

    // Now add comments concurrently
    let num_threads = 8;
    let comments_per_thread = 50;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                let commenter = generate_actor_id();

                barrier.wait();

                let mut success_count = 0;
                for i in 0..comments_per_thread {
                    let ts_offset = ((thread_id * comments_per_thread + i) as u64) + 1;
                    let event = create_comment_event(&commenter, &issue_id, ts_offset);

                    match store.insert_event(&event) {
                        Ok(()) => success_count += 1,
                        Err(e) => {
                            eprintln!("Thread {} comment {} failed: {:?}", thread_id, i, e);
                        }
                    }
                }
                success_count
            })
        })
        .collect();

    let total_success: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let expected = num_threads * comments_per_thread;

    assert_eq!(
        total_success, expected,
        "Expected {} successes, got {}",
        expected, total_success
    );

    // Verify all comments were stored
    let events = store.get_issue_events(&issue_id).expect("Failed to get events");
    // +1 for the create event
    assert_eq!(
        events.len(),
        expected + 1,
        "Expected {} events, got {}",
        expected + 1,
        events.len()
    );
}

#[test]
fn test_concurrent_read_write() {
    let dir = tempdir().unwrap();
    let store = Arc::new(GritStore::open(dir.path()).expect("Failed to open store"));

    let actor = generate_actor_id();
    let issue_id = generate_issue_id();

    // Create initial issue
    let create_event = create_issue_event(&actor, &issue_id, 0);
    store.insert_event(&create_event).expect("Failed to create issue");

    let num_readers = 4;
    let num_writers = 4;
    let ops_per_thread = 100;
    let barrier = Arc::new(Barrier::new(num_readers + num_writers));

    // Spawn reader threads
    let reader_handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                let mut read_count = 0;
                for _ in 0..ops_per_thread {
                    if store.get_issue(&issue_id).is_ok() {
                        read_count += 1;
                    }
                }
                read_count
            })
        })
        .collect();

    // Spawn writer threads
    let writer_handles: Vec<_> = (0..num_writers)
        .map(|thread_id| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                let writer = generate_actor_id();

                barrier.wait();

                let mut write_count = 0;
                for i in 0..ops_per_thread {
                    let ts_offset = ((thread_id * ops_per_thread + i) as u64) + 1;
                    let event = create_comment_event(&writer, &issue_id, ts_offset);

                    if store.insert_event(&event).is_ok() {
                        write_count += 1;
                    }
                }
                write_count
            })
        })
        .collect();

    let total_reads: usize = reader_handles.into_iter().map(|h| h.join().unwrap()).sum();
    let total_writes: usize = writer_handles.into_iter().map(|h| h.join().unwrap()).sum();

    // All reads should succeed
    assert_eq!(
        total_reads,
        num_readers * ops_per_thread,
        "Some reads failed"
    );

    // All writes should succeed
    assert_eq!(
        total_writes,
        num_writers * ops_per_thread,
        "Some writes failed"
    );

    println!(
        "Concurrent read/write: {} reads, {} writes completed",
        total_reads, total_writes
    );
}

#[test]
fn test_rebuild_during_writes() {
    let dir = tempdir().unwrap();
    let store = Arc::new(GritStore::open(dir.path()).expect("Failed to open store"));

    // Create several issues first
    let actor = generate_actor_id();
    for i in 0..10 {
        let issue_id = generate_issue_id();
        let event = create_issue_event(&actor, &issue_id, i as u64);
        store.insert_event(&event).expect("Failed to create issue");
    }

    let barrier = Arc::new(Barrier::new(3));

    // Writer thread
    let store_writer = Arc::clone(&store);
    let barrier_writer = Arc::clone(&barrier);
    let writer_handle = thread::spawn(move || {
        let writer = generate_actor_id();
        barrier_writer.wait();

        let mut success = 0;
        for i in 0..50 {
            let issue_id = generate_issue_id();
            let event = create_issue_event(&writer, &issue_id, (i + 100) as u64);
            if store_writer.insert_event(&event).is_ok() {
                success += 1;
            }
        }
        success
    });

    // Reader thread
    let store_reader = Arc::clone(&store);
    let barrier_reader = Arc::clone(&barrier);
    let reader_handle = thread::spawn(move || {
        barrier_reader.wait();

        let mut success = 0;
        for _ in 0..50 {
            if store_reader.list_issues(&Default::default()).is_ok() {
                success += 1;
            }
        }
        success
    });

    // Rebuild thread
    let store_rebuild = Arc::clone(&store);
    let barrier_rebuild = Arc::clone(&barrier);
    let rebuild_handle = thread::spawn(move || {
        barrier_rebuild.wait();

        // Do a few rebuilds
        let mut success = 0;
        for _ in 0..3 {
            if store_rebuild.rebuild().is_ok() {
                success += 1;
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
        success
    });

    let writes = writer_handle.join().unwrap();
    let reads = reader_handle.join().unwrap();
    let rebuilds = rebuild_handle.join().unwrap();

    // Most operations should succeed
    assert!(writes > 0, "No writes succeeded");
    assert!(reads > 0, "No reads succeeded");
    assert!(rebuilds > 0, "No rebuilds succeeded");

    println!(
        "Rebuild during writes: {} writes, {} reads, {} rebuilds",
        writes, reads, rebuilds
    );
}
