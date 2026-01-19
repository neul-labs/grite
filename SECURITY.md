# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| latest  | :white_check_mark: |

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

### Security Considerations

Grit stores data in git refs and a local sled database. When using Grit, consider:

- **Git repository access**: Anyone with read access to the git repository can read the Grit event log
- **Ed25519 signing**: Enable signing for event authenticity verification
- **Local database**: The sled database is stored in `.git/grit/` and contains materialized views of the event log
- **Daemon IPC**: The daemon uses local IPC sockets for communication

## Security Best Practices

1. Use Ed25519 signing for sensitive workflows
2. Protect your git repository with appropriate access controls
3. Keep your Rust toolchain and dependencies up to date
4. Review the event log periodically for unexpected entries
