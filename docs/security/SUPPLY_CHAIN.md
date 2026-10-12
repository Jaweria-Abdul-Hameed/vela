# Supply-chain gates (X05)

The gates run in `.github/workflows/security.yml` on every push and pull request. Their configuration is data in the
repository, so editing a ticket, an issue or `AGENTS.md` cannot disable one.

| Gate | What it checks | Config |
|---|---|---|
| Secret scan | Added lines and file names in the pushed range: credential-like files (`.env`, key and certificate files, `.gemini` and Antigravity credential paths, cookie databases) and token patterns (cloud keys, GitHub and Slack tokens, private-key blocks, bearer headers, generic assignments). | `security/secret-rules.json`, `scripts/scan-secrets.mjs` |
| Rust advisories, licenses, bans, sources | `cargo deny check` (pinned version, checksum verified). Unknown registries and git sources are denied; licenses outside the allow list fail. | `deny.toml` |
| npm audit | `npm audit --omit=dev --audit-level=high` after `npm ci`. | none |
| Lockfile drift | `cargo metadata --locked` and `npm ci` fail when a manifest and its lockfile disagree. | none |
| SBOM | CycloneDX 1.5 document from `cargo metadata --locked` and `package-lock.json`. | `scripts/generate-sbom.mjs` |

## Exceptions

Every exception lives in `security/audit-allowlist.json` with an `id`, a `scope` (`secrets`, `rust-advisory`,
`npm-advisory`, `license`), a `justification` of at least ten characters and an `expires` date (`YYYY-MM-DD`).
`scripts/check-audit-allowlist.mjs` fails CI for an entry without a justification or an expiry, or past its expiry.
A Rust advisory exception must also be mirrored in `deny.toml` (`advisories.ignore`) with a `reason`.

## Decisions recorded

- The secret scan is a repository script, not the `gitleaks` binary: the rules stay reviewable data, there is no
  third-party action to pin, and a seeded fake secret fails it in the unit tests. `.gitleaks.toml` (named in the
  ticket's write surfaces) was not created; add it only if gitleaks is adopted later.
- The SBOM is generated in CI from the locked graphs and its digest is printed. Publishing it as a release asset belongs
  to the release workflow (Z01, Z06).
