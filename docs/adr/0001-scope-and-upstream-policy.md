# ADR-0001 — Scope and upstream policy

- Status: Accepted — **amended 2026-07-02 by ADR-0002**: the
  upstream-tracking policy below is superseded; `basemind` is a knowledge
  reference, not a tracked upstream (the crate is hand-written
  contract-first). Sovereignty, licensing, and CI gates remain in force.
  The "Presto-Matic" wording predates the current ecosystem naming.
- Date: 2026-06-29
- Upstream: [basemind](https://github.com/Goldziher/basemind)

## Context

`gear-memory` is a companion repository in the Presto-Matic / cos-matic ecosystem. Its role is **local agentic context**. It is intentionally separate from the Presto-Matic product repo so heavy dependencies, operational lifecycle, and upstream tracking stay isolated.

## Decision

Build `gear-memory` as an upstream-first, sovereign Rust project:

- track upstream releases/tags/commits explicitly;
- keep local patches small and temporary;
- expose stable contracts rather than leaking upstream internals to consumers;
- enforce permissive OSS licensing and vulnerability gates in CI;
- default to self-hosted/EU-resident operation and avoid US hyperscaler requirements.

## Integration contract

- CLI/JSON reports for indexing status and queries
- local per-repo cache keyed by repository identity
- explicit import/export for auditable memory entries

## Non-goals

- no hosted SaaS control plane
- no hidden telemetry
- no requirement for Presto-Matic runtime

## Consequences

- The companion can iterate independently from Presto-Matic.
- Presto-Matic avoids accidental dependency bloat and can roll back integration by switching contracts off.
- Upstream changes are absorbed deliberately through version bumps, changelog review, and contract tests.
