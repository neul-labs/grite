//! Regression tests for lock refs under `sync`: re-acquiring a lock, releasing
//! it, and recovering repositories whose lock refs already diverged.
//!
//! Every test uses a bare "origin" and two clones (`a`, `b`), each with its own
//! actor, and drives the real `grite` binary. Lock commands always run
//! in-process; `sync` runs in-process too unless a test opts into the daemon,
//! which then gets its own socket via `GRITE_DAEMON_SOCKET`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use git2::{Oid, Repository, Signature};
use tempfile::TempDir;

static SOCKET_SEQ: AtomicU32 = AtomicU32::new(0);

fn grite_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_grite"))
}

fn daemon_bin() -> PathBuf {
    grite_bin()
        .parent()
        .expect("grite binary has a parent directory")
        .join("grite-daemon")
}

/// The lock ref for `path:src`, matching `libgrite_core::resource_hash`.
fn lock_ref(resource: &str) -> String {
    format!(
        "refs/grite/locks/{}",
        libgrite_core::resource_hash(resource)
    )
}

const RES: &str = "path:src";
const WAL: &str = "refs/grite/wal";

struct Clones {
    _temp: TempDir,
    origin: PathBuf,
    a: PathBuf,
    b: PathBuf,
    socket: PathBuf,
    use_daemon: bool,
}

impl Clones {
    fn new() -> Self {
        let temp = TempDir::new().expect("create temp dir");
        let origin = temp.path().join("origin.git");
        let a = temp.path().join("a");
        let b = temp.path().join("b");

        git(
            &["init", "-q", "--bare", origin.to_str().unwrap()],
            temp.path(),
        );
        git(&["clone", "-q", origin.to_str().unwrap(), "a"], temp.path());
        configure(&a);
        git(&["commit", "-q", "--allow-empty", "-m", "init"], &a);
        git(&["push", "-q", "origin", "HEAD"], &a);
        git(&["clone", "-q", origin.to_str().unwrap(), "b"], temp.path());
        configure(&b);

        let seq = SOCKET_SEQ.fetch_add(1, Ordering::SeqCst);
        let socket = PathBuf::from(format!("/tmp/grite-ls{}-{}.sock", std::process::id(), seq));
        let _ = std::fs::remove_file(&socket);

        let clones = Self {
            _temp: temp,
            origin,
            a,
            b,
            socket,
            use_daemon: false,
        };
        clones.ok(&clones.a, &["init"]);
        clones.ok(&clones.b, &["init"]);
        clones
    }

    fn with_daemon() -> Self {
        let mut clones = Self::new();
        clones.use_daemon = true;
        clones
    }

    fn run(&self, dir: &Path, args: &[&str]) -> Output {
        let mut cmd = Command::new(grite_bin());
        cmd.arg("--json");
        // Lock commands never route through the daemon; only sync is
        // affected by the daemon switch.
        if !self.use_daemon {
            cmd.arg("--no-daemon");
        }
        cmd.args(args)
            .current_dir(dir)
            .env("GRITE_DAEMON_SOCKET", &self.socket)
            .env("GRITE_DAEMON_BIN", daemon_bin())
            .env_remove("XDG_RUNTIME_DIR")
            .output()
            .expect("run grite")
    }

    fn ok(&self, dir: &Path, args: &[&str]) -> serde_json::Value {
        let out = self.run(dir, args);
        let value = json(&out);
        // Daemon-routed commands print the bare payload, without the envelope.
        if value.get("ok").is_none() {
            assert!(out.status.success(), "grite {:?} failed: {:?}", args, out);
            return value;
        }
        assert!(
            out.status.success() && value["ok"] == true,
            "grite {:?} failed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        value["data"].clone()
    }

    fn acquire(&self, dir: &Path, ttl: u64) -> serde_json::Value {
        self.ok(dir, &["lock", "acquire", RES, "--ttl", &ttl.to_string()])
    }

    fn issue(&self, dir: &Path, title: &str) {
        self.ok(dir, &["issue", "create", "--title", title, "--body", "x"]);
    }

    fn push(&self, dir: &Path) -> serde_json::Value {
        self.ok(dir, &["sync", "--push"])
    }

    fn pull(&self, dir: &Path) -> serde_json::Value {
        self.ok(dir, &["sync", "--pull"])
    }

    fn sync(&self, dir: &Path) -> serde_json::Value {
        self.ok(dir, &["sync"])
    }
}

impl Drop for Clones {
    fn drop(&mut self) {
        if self.use_daemon {
            let _ = Command::new(grite_bin())
                .args(["--json", "--quiet", "daemon", "stop"])
                .current_dir(&self.a)
                .env("GRITE_DAEMON_SOCKET", &self.socket)
                .env("GRITE_DAEMON_BIN", daemon_bin())
                .output();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn configure(dir: &Path) {
    git(&["config", "user.email", "test@example.com"], dir);
    git(&["config", "user.name", "Test"], dir);
}

fn git(args: &[&str], dir: &Path) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn json(out: &Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "expected JSON on stdout, got {:?} (stderr: {}) — {}",
            stdout,
            String::from_utf8_lossy(&out.stderr),
            e
        )
    })
}

fn tip(repo: &Path, name: &str) -> Option<Oid> {
    Repository::open(repo)
        .expect("open repo")
        .refname_to_id(name)
        .ok()
}

/// Owner recorded in the lock ref `name` of `repo`, if the ref exists.
fn lock_owner(repo: &Path, name: &str) -> Option<String> {
    let repo = Repository::open(repo).expect("open repo");
    let oid = repo.refname_to_id(name).ok()?;
    let tree = repo.find_commit(oid).unwrap().tree().unwrap();
    let blob = repo
        .find_blob(tree.get_name("lock.json").unwrap().id())
        .unwrap();
    let lock: serde_json::Value = serde_json::from_slice(blob.content()).unwrap();
    Some(lock["owner"].as_str().unwrap().to_string())
}

/// Write `refs/grite/locks/<hash>` the way grite <= 0.5.3 did: a parentless
/// commit holding `lock.json`. Used to build the already-broken state.
fn write_legacy_lock(repo: &Path, owner: &str, expires_unix_ms: u64) -> Oid {
    let repo = Repository::open(repo).expect("open repo");
    let json = serde_json::json!({
        "owner": owner,
        "nonce": uuid_like(expires_unix_ms, owner),
        "expires_unix_ms": expires_unix_ms,
        "resource": RES,
    });
    let blob = repo
        .blob(serde_json::to_string_pretty(&json).unwrap().as_bytes())
        .unwrap();
    let mut tb = repo.treebuilder(None).unwrap();
    tb.insert("lock.json", blob, 0o100644).unwrap();
    let tree = repo.find_tree(tb.write().unwrap()).unwrap();
    let sig = Signature::now("grite", "grit@localhost").unwrap();
    let oid = repo
        .commit(None, &sig, &sig, &format!("Lock: {}", RES), &tree, &[])
        .unwrap();
    repo.reference(&lock_ref(RES), oid, true, "legacy lock")
        .unwrap();
    oid
}

fn uuid_like(n: u64, owner: &str) -> String {
    format!("legacy-{}-{}", owner, n)
}

fn expire() {
    std::thread::sleep(Duration::from_millis(1200));
}

/// Item 3 of the report, same actor: once origin has the lock ref, re-acquiring
/// after expiry used to write an unrelated root commit, and the atomic push then
/// failed `NotFastForward` and took the (fast-forwardable) WAL down with it.
#[test]
fn reacquire_after_expiry_pushes_cleanly() {
    let c = Clones::new();
    c.acquire(&c.a, 1);
    c.issue(&c.a, "one");
    assert_eq!(c.push(&c.a)["success"], true);

    expire();
    c.acquire(&c.a, 60);
    c.issue(&c.a, "two");
    let pushed = c.push(&c.a);

    assert_eq!(pushed["success"], true, "{pushed}");
    assert_eq!(tip(&c.origin, WAL), tip(&c.a, WAL), "WAL reached origin");
    assert_eq!(
        tip(&c.origin, &lock_ref(RES)),
        tip(&c.a, &lock_ref(RES)),
        "re-acquired lock reached origin"
    );
}

/// Item 3, another actor: B takes over A's expired lock, with and without
/// having pulled A's lock first.
#[test]
fn another_actor_takes_over_an_expired_lock() {
    for pull_first in [true, false] {
        let c = Clones::new();
        let a_owner = c.acquire(&c.a, 1)["owner"].as_str().unwrap().to_string();
        c.push(&c.a);
        if pull_first {
            c.pull(&c.b);
            assert_eq!(lock_owner(&c.b, &lock_ref(RES)), Some(a_owner));
        }

        expire();
        let b_owner = c.acquire(&c.b, 60)["owner"].as_str().unwrap().to_string();
        c.issue(&c.b, "from b");
        let pushed = c.push(&c.b);

        assert_eq!(pushed["success"], true, "pull_first={pull_first}: {pushed}");
        assert_eq!(tip(&c.origin, WAL), tip(&c.b, WAL));
        assert_eq!(
            lock_owner(&c.origin, &lock_ref(RES)),
            Some(b_owner.clone()),
            "pull_first={pull_first}: origin holds B's lock"
        );

        // A sees the takeover and can no longer acquire.
        c.pull(&c.a);
        assert_eq!(lock_owner(&c.a, &lock_ref(RES)), Some(b_owner));
        let out = c.run(&c.a, &["lock", "acquire", RES, "--ttl", "60"]);
        assert!(!out.status.success(), "A must not acquire B's live lock");
    }
}

/// The negative case: a LIVE lock held by another actor still conflicts across
/// clones, and that conflict no longer blocks the WAL.
#[test]
fn live_lock_of_another_actor_still_conflicts() {
    let c = Clones::new();
    let a_owner = c.acquire(&c.a, 600)["owner"].as_str().unwrap().to_string();
    c.issue(&c.a, "from a");
    c.push(&c.a);
    let origin_lock = tip(&c.origin, &lock_ref(RES));

    // B has not pulled, so its local acquire succeeds against a stale view,
    // and its WAL has diverged from origin's as well.
    c.acquire(&c.b, 600);
    c.issue(&c.b, "from b");
    let pushed = c.push(&c.b);

    assert_eq!(pushed["success"], true, "WAL push succeeds: {pushed}");
    assert_eq!(tip(&c.origin, WAL), tip(&c.b, WAL), "WAL reached origin");
    assert_eq!(
        tip(&c.origin, &lock_ref(RES)),
        origin_lock,
        "A's live lock on origin is untouched"
    );
    let conflicts = pushed["lock_conflicts"].as_array().expect("lock_conflicts");
    assert_eq!(conflicts.len(), 1, "conflict reported: {pushed}");
    assert_eq!(conflicts[0]["resource"], RES);
    assert_eq!(conflicts[0]["owner"], a_owner.as_str());

    // B's view now reflects A's lock, so B is refused locally.
    assert_eq!(lock_owner(&c.b, &lock_ref(RES)), Some(a_owner));
    let out = c.run(&c.b, &["lock", "acquire", RES, "--ttl", "60"]);
    assert!(!out.status.success(), "B must not acquire A's live lock");
}

/// Item 5: release propagates to origin, so a pull does not bring it back.
#[test]
fn release_propagates_to_origin() {
    let c = Clones::new();
    c.acquire(&c.a, 600);
    c.push(&c.a);
    c.pull(&c.b);
    assert!(tip(&c.b, &lock_ref(RES)).is_some());

    c.ok(&c.a, &["lock", "release", RES]);
    c.sync(&c.a);
    assert_eq!(tip(&c.origin, &lock_ref(RES)), None, "origin ref deleted");

    // The release sticks for A across a pull.
    c.pull(&c.a);
    let status = c.ok(&c.a, &["lock", "status"]);
    assert_eq!(status["total"], 0, "released lock came back: {status}");

    // B's copy of A's lock goes too, so B can take it straight away.
    c.pull(&c.b);
    let status = c.ok(&c.b, &["lock", "status"]);
    assert_eq!(
        status["total"], 0,
        "B still sees the released lock: {status}"
    );
    c.acquire(&c.b, 1);
    c.push(&c.b);
    expire();

    // A can lock again and push it; nothing diverges.
    c.acquire(&c.a, 600);
    assert_eq!(c.push(&c.a)["success"], true);
    assert_eq!(tip(&c.origin, &lock_ref(RES)), tip(&c.a, &lock_ref(RES)));
}

/// Item 5, gc: expired locks collected locally are also removed from origin,
/// and do not come back on the next pull.
#[test]
fn gc_and_expiry_propagate_to_origin() {
    let c = Clones::new();
    c.acquire(&c.a, 1);
    c.push(&c.a);
    expire();

    let gc = c.ok(&c.a, &["lock", "gc"]);
    assert_eq!(gc["removed"], 1);
    c.sync(&c.a);
    assert_eq!(tip(&c.origin, &lock_ref(RES)), None, "origin ref deleted");
    assert_eq!(tip(&c.a, &lock_ref(RES)), None, "not pulled back");
}

/// Item 4: `sync --pull` reconciles a diverged, expired local lock ref.
#[test]
fn pull_reconciles_a_diverged_expired_lock() {
    let c = Clones::new();
    c.acquire(&c.a, 600);
    c.push(&c.a);
    let origin_lock = tip(&c.origin, &lock_ref(RES));

    // B holds an unrelated, expired lock ref for the same resource.
    write_legacy_lock(&c.b, "00000000000000000000000000000000", 1);
    c.pull(&c.b);

    assert_eq!(
        tip(&c.b, &lock_ref(RES)),
        origin_lock,
        "B adopted origin's lock"
    );
}

/// The state real repositories are already in: unrelated, expired lock refs on
/// both origin and a clone. A normal sync recovers without manual ref surgery,
/// and the WAL is pushed in the same sync.
#[test]
fn diverged_expired_locks_self_heal_on_sync() {
    let c = Clones::new();
    c.issue(&c.a, "seed");
    c.push(&c.a);
    write_legacy_lock(&c.a, "11111111111111111111111111111111", 1);
    c.ok(&c.a, &["sync", "--push"]);
    write_legacy_lock(&c.a, "22222222222222222222222222222222", 2);
    assert_ne!(tip(&c.a, &lock_ref(RES)), tip(&c.origin, &lock_ref(RES)));

    c.issue(&c.a, "after");
    let synced = c.sync(&c.a);

    assert_eq!(synced["push_success"], true, "{synced}");
    assert_eq!(tip(&c.origin, WAL), tip(&c.a, WAL), "WAL reached origin");
    assert_eq!(tip(&c.origin, &lock_ref(RES)), None, "stale lock gone");
    assert_eq!(tip(&c.a, &lock_ref(RES)), None, "stale lock gone locally");

    // And locking works normally afterwards.
    c.acquire(&c.a, 600);
    assert_eq!(c.push(&c.a)["success"], true);
    assert_eq!(tip(&c.origin, &lock_ref(RES)), tip(&c.a, &lock_ref(RES)));
}

/// A legacy root-commit lock on origin that is still live is respected: a
/// healing sync never replaces another actor's live lock.
#[test]
fn self_heal_never_replaces_a_live_legacy_lock() {
    let c = Clones::new();
    let far_future = 32_503_680_000_000; // year 3000
    write_legacy_lock(&c.origin, "33333333333333333333333333333333", far_future);
    let origin_lock = tip(&c.origin, &lock_ref(RES));

    write_legacy_lock(&c.b, "44444444444444444444444444444444", 1);
    c.acquire(&c.b, 600);
    let pushed = c.push(&c.b);

    assert_eq!(tip(&c.origin, &lock_ref(RES)), origin_lock);
    assert_eq!(pushed["lock_conflicts"].as_array().map(Vec::len), Some(1));
}

/// Divergent WAL histories in two clones are rebased by `sync`, which used to
/// fail outright with `NotFastForward`.
#[test]
fn diverged_wal_is_rebased_on_sync() {
    let c = Clones::new();
    c.issue(&c.a, "from a");
    c.push(&c.a);
    c.issue(&c.b, "from b");

    let synced = c.sync(&c.b);

    assert_eq!(synced["push_success"], true, "{synced}");
    assert_eq!(tip(&c.origin, WAL), tip(&c.b, WAL));
    c.pull(&c.a);
    assert_eq!(tip(&c.a, WAL), tip(&c.origin, WAL));
}

/// The daemon runs sync through the same `SyncManager`; cover the original
/// repro on that path too.
#[test]
fn reacquire_after_expiry_pushes_cleanly_through_the_daemon() {
    if !daemon_bin().exists() {
        panic!(
            "grite-daemon not found at {}; run `cargo test --all`",
            daemon_bin().display()
        );
    }
    let c = Clones::with_daemon();
    c.acquire(&c.a, 1);
    c.issue(&c.a, "one");
    c.push(&c.a);

    expire();
    c.acquire(&c.a, 60);
    c.issue(&c.a, "two");
    let pushed = c.push(&c.a);

    assert_eq!(pushed["push_success"], true, "{pushed}");
    assert_eq!(tip(&c.origin, WAL), tip(&c.a, WAL));
    assert_eq!(tip(&c.origin, &lock_ref(RES)), tip(&c.a, &lock_ref(RES)));
}

/// Linked worktrees share lock refs. A sibling worktree's unpushed lock is not
/// mistaken for a stale copy of a remote lock: another actor's sync keeps it,
/// and publishes it.
#[test]
fn sibling_worktree_lock_survives_a_sync_from_another_actor() {
    let c = Clones::new();
    let wt = c.a.parent().unwrap().join("a-wt");
    git(
        &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "wt"],
        &c.a,
    );
    c.ok(&wt, &["actor", "init", "--label", "wt"]);

    // The worktree locks; the main checkout (a different actor) syncs.
    let wt_owner = c.acquire(&wt, 600)["owner"].as_str().unwrap().to_string();
    c.sync(&c.a);

    assert_eq!(lock_owner(&c.a, &lock_ref(RES)), Some(wt_owner.clone()));
    assert_eq!(lock_owner(&c.origin, &lock_ref(RES)), Some(wt_owner));
}
