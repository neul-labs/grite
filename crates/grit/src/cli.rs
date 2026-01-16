use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "grit", about = "Git-backed issue tracking", version)]
pub struct Cli {
    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress human-readable output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Override the data directory
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    /// Override the actor ID
    #[arg(long, global = true)]
    pub actor: Option<String>,

    /// Force local execution (skip daemon)
    #[arg(long, global = true)]
    pub no_daemon: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize grit in the current repository
    Init,

    /// Actor management commands
    Actor {
        #[command(subcommand)]
        cmd: ActorCommand,
    },

    /// Issue management commands
    Issue {
        #[command(subcommand)]
        cmd: IssueCommand,
    },

    /// Database management commands
    Db {
        #[command(subcommand)]
        cmd: DbCommand,
    },

    /// Export issues to file
    Export {
        /// Export format
        #[arg(long)]
        format: ExportFormat,

        /// Export changes since timestamp or event ID
        #[arg(long)]
        since: Option<String>,
    },

    /// Rebuild local database from events
    Rebuild,

    /// Sync with remote repository
    Sync {
        /// Remote name (default: origin)
        #[arg(long, default_value = "origin")]
        remote: String,

        /// Pull only (don't push)
        #[arg(long)]
        pull: bool,

        /// Push only (don't pull)
        #[arg(long)]
        push: bool,
    },

    /// Snapshot management
    Snapshot {
        #[command(subcommand)]
        cmd: SnapshotCommand,
    },

    /// Daemon management
    Daemon {
        #[command(subcommand)]
        cmd: DaemonCommand,
    },

    /// Lock management for team coordination
    Lock {
        #[command(subcommand)]
        cmd: LockCommand,
    },
}

#[derive(Clone, Subcommand)]
pub enum ActorCommand {
    /// Create a new actor
    Init {
        /// Human-friendly label for the actor
        #[arg(long)]
        label: Option<String>,

        /// Generate Ed25519 signing key for this actor
        #[arg(long)]
        generate_key: bool,
    },

    /// List all actors
    List,

    /// Show actor details
    Show {
        /// Actor ID (defaults to current)
        id: Option<String>,
    },

    /// Show current actor
    Current,

    /// Set the default actor
    Use {
        /// Actor ID to use as default
        id: String,
    },
}

#[derive(Clone, Subcommand)]
pub enum IssueCommand {
    /// Create a new issue
    Create {
        /// Issue title
        #[arg(long)]
        title: String,

        /// Issue body
        #[arg(long, default_value = "")]
        body: String,

        /// Labels to add
        #[arg(long)]
        label: Vec<String>,
    },

    /// List issues
    List {
        /// Filter by state
        #[arg(long)]
        state: Option<String>,

        /// Filter by label
        #[arg(long)]
        label: Option<String>,
    },

    /// Show issue details
    Show {
        /// Issue ID
        id: String,
    },

    /// Update an issue
    Update {
        /// Issue ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New body
        #[arg(long)]
        body: Option<String>,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },

    /// Add a comment to an issue
    Comment {
        /// Issue ID
        id: String,

        /// Comment body
        #[arg(long)]
        body: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },

    /// Close an issue
    Close {
        /// Issue ID
        id: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },

    /// Reopen an issue
    Reopen {
        /// Issue ID
        id: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },

    /// Label operations
    Label {
        #[command(subcommand)]
        cmd: LabelCommand,
    },

    /// Assignee operations
    Assignee {
        #[command(subcommand)]
        cmd: AssigneeCommand,
    },

    /// Link operations
    Link {
        #[command(subcommand)]
        cmd: LinkCommand,
    },

    /// Attachment operations
    Attachment {
        #[command(subcommand)]
        cmd: AttachmentCommand,
    },
}

#[derive(Clone, Subcommand)]
pub enum LabelCommand {
    /// Add a label to an issue
    Add {
        /// Issue ID
        id: String,

        /// Label to add
        #[arg(long)]
        label: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },

    /// Remove a label from an issue
    Remove {
        /// Issue ID
        id: String,

        /// Label to remove
        #[arg(long)]
        label: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },
}

#[derive(Clone, Subcommand)]
pub enum AssigneeCommand {
    /// Add an assignee to an issue
    Add {
        /// Issue ID
        id: String,

        /// User to assign
        #[arg(long)]
        user: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },

    /// Remove an assignee from an issue
    Remove {
        /// Issue ID
        id: String,

        /// User to unassign
        #[arg(long)]
        user: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },
}

#[derive(Clone, Subcommand)]
pub enum LinkCommand {
    /// Add a link to an issue
    Add {
        /// Issue ID
        id: String,

        /// URL to link
        #[arg(long)]
        url: String,

        /// Optional note
        #[arg(long)]
        note: Option<String>,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },
}

#[derive(Clone, Subcommand)]
pub enum AttachmentCommand {
    /// Add an attachment reference to an issue
    Add {
        /// Issue ID
        id: String,

        /// Attachment name
        #[arg(long)]
        name: String,

        /// SHA256 hash of the attachment
        #[arg(long)]
        sha256: String,

        /// MIME type
        #[arg(long)]
        mime: String,

        /// Acquire lock before operation, release after
        #[arg(long)]
        lock: bool,
    },
}

#[derive(Clone, Subcommand)]
pub enum DbCommand {
    /// Show database statistics
    Stats,

    /// Check database integrity (hash verification)
    Check {
        /// Also verify parent event references
        #[arg(long)]
        verify_parents: bool,
    },

    /// Verify event signatures
    Verify {
        /// Show details for each event
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Clone, Subcommand)]
pub enum SnapshotCommand {
    /// Create a new snapshot
    Create,

    /// List all snapshots
    List,

    /// Garbage collect old snapshots
    Gc {
        /// Number of snapshots to keep
        #[arg(long, default_value = "5")]
        keep: usize,
    },
}

#[derive(Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Md,
}

#[derive(Clone, Subcommand)]
pub enum DaemonCommand {
    /// Show daemon status
    Status,

    /// Stop the daemon
    Stop,
}

#[derive(Clone, Subcommand)]
pub enum LockCommand {
    /// Acquire a lock on a resource
    Acquire {
        /// Resource to lock (e.g., "repo:global", "issue:abc123", "path:src/")
        resource: String,

        /// Lock duration in seconds (default: 300)
        #[arg(long, default_value = "300")]
        ttl: u64,
    },

    /// Release a lock
    Release {
        /// Resource to unlock
        resource: String,
    },

    /// Show lock status
    Status,

    /// Garbage collect expired locks
    Gc,
}
