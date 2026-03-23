//! Integration test for concurrent requests to the daemon
//!
//! Verifies that when multiple clients connect simultaneously, all requests
//! are handled concurrently without head-of-line blocking or timeouts.

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use libgrite_ipc::framing::{read_framed, write_framed};
use libgrite_ipc::messages::IpcResponse;
use libgrite_ipc::{IpcCommand, IpcRequest};

/// Create a minimal git repo with grite actor initialized
fn setup_repo(dir: &Path) -> (String, String) {
    // Init git repo
    assert!(Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success());

    assert!(Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success());

    assert!(Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success());

    // Create actor directory structure
    let actor_id = "00112233445566778899aabbccddeeff";
    let actor_dir = dir.join(".git/grite/actors").join(actor_id);
    std::fs::create_dir_all(&actor_dir).unwrap();

    // Write actor config
    let config_content = format!(
        "actor_id = \"{}\"\nlabel = \"test\"\n",
        actor_id
    );
    std::fs::write(actor_dir.join("config.toml"), config_content).unwrap();

    let repo_root = dir.to_string_lossy().to_string();
    let data_dir = actor_dir.to_string_lossy().to_string();
    (repo_root, data_dir)
}

/// Send a single IPC request over a Unix socket and return the response
fn send_request(
    socket_path: &str,
    repo_root: &str,
    actor_id: &str,
    data_dir: &str,
    request_id: &str,
    command: IpcCommand,
) -> Result<IpcResponse, String> {
    let mut stream = UnixStream::connect(socket_path).map_err(|e| format!("connect: {}", e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let request = IpcRequest::new(
        request_id.to_string(),
        repo_root.to_string(),
        actor_id.to_string(),
        data_dir.to_string(),
        command,
    );

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&request)
        .map_err(|e| format!("serialize: {}", e))?;

    write_framed(&mut stream, &bytes).map_err(|e| format!("write: {}", e))?;

    let response_bytes = read_framed(&mut stream).map_err(|e| format!("read: {}", e))?;

    let archived =
        rkyv::access::<rkyv::Archived<IpcResponse>, rkyv::rancor::Error>(&response_bytes)
            .map_err(|e| format!("access: {}", e))?;

    rkyv::deserialize::<IpcResponse, rkyv::rancor::Error>(archived)
        .map_err(|e| format!("deserialize: {}", e))
}

/// Start a supervisor and wait for the socket to appear
async fn start_supervisor(
    socket_path: String,
) -> tokio::task::JoinHandle<()> {
    use grite_daemon::supervisor::Supervisor;

    let sp = socket_path.clone();
    let handle = tokio::spawn(async move {
        let mut supervisor = Supervisor::new(sp, None);
        if let Err(e) = supervisor.run().await {
            eprintln!("Supervisor error: {}", e);
        }
    });

    // Wait for socket to appear
    let start = Instant::now();
    while !std::path::Path::new(&socket_path).exists()
        && start.elapsed() < Duration::from_secs(5)
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    handle
}

/// Stop the supervisor by sending DaemonStop
fn stop_supervisor(socket_path: &str, repo_root: &str, actor_id: &str, data_dir: &str) {
    let _ = send_request(
        socket_path,
        repo_root,
        actor_id,
        data_dir,
        "stop",
        IpcCommand::DaemonStop,
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_single_request_succeeds() {
    let temp = tempfile::tempdir().unwrap();
    let (repo_root, data_dir) = setup_repo(temp.path());
    let actor_id = "00112233445566778899aabbccddeeff";
    let socket_path = temp.path().join("daemon.sock");
    let socket_str = socket_path.to_string_lossy().to_string();

    let supervisor = start_supervisor(socket_str.clone()).await;

    let result = send_request(
        &socket_str,
        &repo_root,
        actor_id,
        &data_dir,
        "single-req",
        IpcCommand::IssueList {
            state: Some("open".to_string()),
            label: None,
        },
    );

    match &result {
        Ok(response) => {
            assert!(response.ok, "Response should be ok: {:?}", response.error);
        }
        Err(e) => {
            panic!("Single request failed: {}", e);
        }
    }

    stop_supervisor(&socket_str, &repo_root, actor_id, &data_dir);
    let _ = tokio::time::timeout(Duration::from_secs(5), supervisor).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_requests_all_succeed() {
    let temp = tempfile::tempdir().unwrap();
    let (repo_root, data_dir) = setup_repo(temp.path());
    let actor_id = "00112233445566778899aabbccddeeff";
    let socket_path = temp.path().join("daemon.sock");
    let socket_str = socket_path.to_string_lossy().to_string();

    let supervisor = start_supervisor(socket_str.clone()).await;

    // Send N concurrent requests from separate threads
    let n = 5;
    let barrier = Arc::new(Barrier::new(n));
    let mut handles = vec![];

    for i in 0..n {
        let sp = socket_str.clone();
        let rr = repo_root.clone();
        let dd = data_dir.clone();
        let aid = actor_id.to_string();
        let barrier = barrier.clone();

        let handle = std::thread::spawn(move || {
            // Synchronize all threads to connect simultaneously
            barrier.wait();

            let req_id = format!("concurrent-req-{}", i);
            let start = Instant::now();
            let result = send_request(
                &sp,
                &rr,
                &aid,
                &dd,
                &req_id,
                IpcCommand::IssueList {
                    state: Some("open".to_string()),
                    label: None,
                },
            );
            let elapsed = start.elapsed();

            (i, result, elapsed)
        });
        handles.push(handle);
    }

    // Collect results
    let mut successes = 0;
    for handle in handles {
        let (i, result, elapsed) = handle.join().expect("thread panicked");
        match result {
            Ok(response) => {
                assert!(
                    response.ok,
                    "Request {} should succeed, got error: {:?}",
                    i, response.error
                );
                assert!(
                    elapsed < Duration::from_secs(15),
                    "Request {} took too long: {:?}",
                    i,
                    elapsed
                );
                successes += 1;
            }
            Err(e) => {
                panic!("Request {} failed: {}", i, e);
            }
        }
    }

    assert_eq!(successes, n, "All {} requests should succeed", n);

    stop_supervisor(&socket_str, &repo_root, actor_id, &data_dir);
    let _ = tokio::time::timeout(Duration::from_secs(5), supervisor).await;
}
