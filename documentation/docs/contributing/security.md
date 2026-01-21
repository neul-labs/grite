# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| latest | Yes |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

1. **Do not** open a public GitHub issue for security vulnerabilities
2. Email the maintainers directly with details of the vulnerability
3. Include the following information:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes (optional)

### What to Expect

- Acknowledgment of your report within 48 hours
- Regular updates on the progress of addressing the vulnerability
- Credit in the security advisory (if desired) once the issue is resolved

## Security Considerations

Grit stores data in git refs and a local sled database. When using Grit, consider:

### Git Repository Access

Anyone with read access to the git repository can read the Grit event log. This includes:

- Issue titles and bodies
- Comments
- Labels and assignees
- Event timestamps and actor IDs

**Recommendation:** Use repository access controls to protect sensitive issues.

### Ed25519 Signing

Enable signing for event authenticity verification:

```bash
grit actor init --generate-key
```

This creates:

- Private key stored locally (never synced)
- Public key stored in config (synced with actor info)
- All events signed automatically

### Local Database

The sled database is stored in `.git/grit/actors/<id>/sled/` and contains:

- Materialized views of all events
- Indexed issue data

**Recommendation:** Protect the `.git/` directory with appropriate file permissions.

### Daemon IPC

The daemon uses local IPC sockets for communication:

- Socket at `ipc:///tmp/gritd.sock`
- Local machine only
- No network exposure

### Signing Keys

Private signing keys are stored at:

```
.git/grit/actors/<actor_id>/keys/signing.key
```

**Important:**

- Never commit signing keys
- Never share signing keys
- Set appropriate file permissions:
  ```bash
  chmod 600 .git/grit/actors/*/keys/signing.key
  ```

## Security Best Practices

### 1. Use Ed25519 Signing

For sensitive workflows, enable event signing:

```bash
grit actor init --label "secure-actor" --generate-key
```

Verify signatures:

```bash
grit db verify --verbose --json
```

### 2. Protect Repository Access

- Use private repositories for sensitive issues
- Limit access to security-related issues
- Review access permissions regularly

### 3. Keep Dependencies Updated

- Update Rust toolchain regularly
- Check for security advisories in dependencies
- Run `cargo audit` periodically

### 4. Review Event Log

Periodically review the event log for unexpected entries:

```bash
grit issue list --json | jq '.data.issues[] | {title, actor: .events[0].actor}'
```

### 5. Use Lock Policies

For critical operations, enable lock requirements:

```toml
# .git/grit/config.toml
lock_policy = "require"
```

### 6. Isolate Sensitive Work

Use separate actors for sensitive work:

```bash
grit actor init --label "security-audit" --generate-key
grit actor use <security-actor-id>
```

## Security Features

### Event Integrity

- Events are content-addressed (BLAKE2b-256 hash)
- Any modification changes the event ID
- Tampering is detectable

### Append-Only Log

- Events cannot be deleted or modified
- Full history preserved
- Audit trail by design

### Deterministic Merging

- CRDT semantics prevent conflicts
- No opportunity for merge-based attacks
- Consistent state across all actors

## Threat Model

### In Scope

- Event integrity (hash-based)
- Event authenticity (optional signatures)
- Actor isolation
- Local database protection

### Out of Scope

- Git transport security (handled by git)
- Network security (handled by git remotes)
- Operating system security
- Physical security

## Contact

For security concerns, contact the maintainers through the repository's security contact or private channels.
