# memory-card

Local agentic context layer: code map, repo memory, and document/search substrate for coding agents.

> Status: `0.0.0` skeleton — public repo created so the boundary, upstream policy,
> and CI gates are explicit before implementation starts.

## Why it exists

Agents need durable, local context without burning model context windows or depending on a hosted service.

## Upstream relationship

- Upstream: [basemind](https://github.com/Goldziher/basemind)
- Policy: upstream-first, pinned releases/commits, no permanent fork.
- Fork rule: fork only for a blocking security/build/sovereignty patch; open the upstream PR and remove the fork once merged.

## Contract shape

- CLI/JSON reports for indexing status and queries
- local per-repo cache keyed by repository identity
- explicit import/export for auditable memory entries

## Non-goals

- no hosted SaaS control plane
- no hidden telemetry
- no requirement for Presto-Matic runtime

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
```

## License

MIT © Constantin Jais
