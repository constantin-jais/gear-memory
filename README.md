# gear-memory

> Local agentic context layer — code map, repo memory, and search substrate for coding agents. No hosted services, no hidden telemetry.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/Rust-1.95%2B-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/constantin-jais/gear-memory/actions/workflows/ci.yml/badge.svg)](https://github.com/constantin-jais/gear-memory/actions/workflows/ci.yml)

> **Status:** `0.0.0` skeleton — boundary, upstream policy, and CI gates are explicit before implementation starts.

## Why it exists

Agents need durable, local context without burning model context windows or depending on a hosted service. `gear-memory` provides a transparent, per-repo cache that agents query for code maps, document indexes, and history — fully auditable and portable.

## Ecosystem

```mermaid
graph TB
    subgraph products["🎯 Rumble products"]
        RL["rumble-lm<br/>Collaborative learning platform"]
        RC["rumble-cos<br/>Public knowledge site"]
    end
    subgraph tools["🛠️ Sovereign tooling"]
        CM["cos-matic<br/>Agent config + autonomous code-ops"]
        WL["wrench-loader<br/>Document ingestion worker"]
        GM["gear-memory<br/>Local agent context"]
    end
    subgraph infra["⚙️ Infrastructure"]
        GC["gear-cable<br/>Distribution substrate"]
        GD["gear-depot<br/>Registry proxy/cache"]
        VI["vault-inspector<br/>Postgres security audit"]
    end
    RL --> WL
    RL --> GM
    RL --> VI
    RL --> GD
    RL --> GC
    RC --> RL
    CM --> GC
    WL --> GM
    style GM fill:#dbeafe,stroke:#2563eb,stroke-width:2px
```

## Contract

|                 |                                                      |
| --------------- | ---------------------------------------------------- |
| **Interface**   | CLI and JSON reports for indexing status and queries |
| **Storage**     | Local per-repo cache keyed by repository identity    |
| **Portability** | Explicit import/export for auditable memory entries  |

## Non-goals

- No hosted SaaS control plane
- No hidden telemetry
- No requirement for rumble-lm runtime

## Upstream

|               |                                                                                                            |
| ------------- | ---------------------------------------------------------------------------------------------------------- |
| **Project**   | [basemind](https://github.com/Goldziher/basemind)                                                          |
| **Policy**    | Upstream-first, pinned releases/commits, no permanent fork                                                 |
| **Fork rule** | Only for a blocking security/build/sovereignty patch; open the upstream PR and remove the fork once merged |

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
```

## Related projects

| Repo                                                                  | Role                                                 |
| --------------------------------------------------------------------- | ---------------------------------------------------- |
| [rumble-lm](https://github.com/constantin-jais/rumble-lm)          | Primary consumer — agent context for RAG and codegen |
| [wrench-loader](https://github.com/constantin-jais/wrench-loader)         | Feeds extracted document text into gear-memory       |
| [cos-matic](https://github.com/constantin-jais/cos-matic)     | Config compiler and autonomous orchestrator          |
| [gear-cable](https://github.com/constantin-jais/gear-cable)           | Multi-platform distribution substrate                |
| [gear-depot](https://github.com/constantin-jais/gear-depot)       | Sovereign registry proxy / cache                     |
| [vault-inspector](https://github.com/constantin-jais/vault-inspector) | Postgres security audit                              |

## License

MIT © Constantin Jais
