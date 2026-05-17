# Security Policy

## Supported versions

`usta` is pre-1.0. Only the **latest minor release** receives security fixes.
Once we ship 1.0, we'll widen this to the previous minor as well.

| Version | Status |
|---------|--------|
| 0.1.x   | ✅ supported |
| < 0.1   | ❌ pre-release; please upgrade |

## Reporting a vulnerability

**Please do not file a public issue for security problems.** Instead, use
GitHub's private vulnerability reporting:

1. Go to the [Security tab](https://github.com/sunduq-ai/usta-cli/security/advisories/new)
   on this repository.
2. Click **"Report a vulnerability"**.
3. Fill in details: affected version, reproduction, impact, suggested fix
   if any.

If you cannot use GitHub's flow, email **mo7amed.3bdalla7@gmail.com** with
`[usta-cli security]` in the subject line. Encrypt with our public key
(coming for v0.1.0) if the report contains sensitive data.

## Response SLA

| Stage | Target |
|-------|--------|
| Acknowledge receipt | 72 hours |
| Initial assessment | 7 days |
| Coordinated fix + release | 30 days for high-severity, longer for low |
| Public disclosure | After a fix ships, or 90 days from report (whichever is sooner) |

## What's in scope

- Path traversal in the local filesystem adapter (the write-jail is the
  single most important safety property; covered by a `proptest` property
  test, but please report any way to defeat it).
- Template execution producing files outside the resolved output dir.
- `usta extract` reading or writing files outside the configured roots.
- Crashes triggered by malformed `template.toml` / `feature.toml` /
  `.usta-extract.toml` that affect host integrity (DoS via OOM, panics
  with sensitive data in the message, etc.).
- Supply-chain concerns in our own dependencies (we'll triage and update
  even when we're not directly affected).

## What's out of scope

- Bugs that require an attacker who can already write to the templates
  directory or the project root. (Templates are user-controlled; we treat
  them as trusted input the same way `make` treats your `Makefile`.)
- Issues that depend on a malicious local user with the ability to run
  arbitrary commands as you.
- Vulnerabilities in third-party templates published by the community
  (please report to the template author).
- Anything in [`docs/NON_GOALS.md`](./docs/NON_GOALS.md). In particular,
  there's no telemetry, no hosted service, and no LLM API call surface to
  attack — by design.

## Hall of fame

We'll list reporters who request public credit here once we have any.

## Bounty

We don't run a paid bug bounty. We do say thank you, list you in the hall
of fame, and respond fast.
