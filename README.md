# Gear Memory

[![CI](https://github.com/constantin-jais/gear-memory/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/constantin-jais/gear-memory/actions/workflows/ci.yml)
[![Security](https://github.com/constantin-jais/gear-memory/actions/workflows/security.yml/badge.svg?branch=main)](https://github.com/constantin-jais/gear-memory/actions/workflows/security.yml)
[![Contracts](https://github.com/constantin-jais/gear-memory/actions/workflows/contracts.yml/badge.svg?branch=main)](https://github.com/constantin-jais/gear-memory/actions/workflows/contracts.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Layer:** Gear — Infrastructure  
**Role:** local context and memory substrate  
**Mission:** make local-first source, memory, code graph, event, and provenance references trustworthy without deciding what agents or products should do next.

---

## Stack role

- **Layer:** Gear — Infrastructure.
- **Role:** local context and memory substrate.
- **Mission:** make local-first source, memory, code graph, event, and provenance references trustworthy without deciding what agents or products should do next.
- **Maturity:** `contract-first`.
- **Scale-ready:** no — contracts/tests exist, but local persistence and indexing are not proven yet.
- **Current increment:** P0 memory/source/provenance contracts.
- **Learning value:** local-first memory, source refs, event logs, code maps, and privacy-preserving context.
- **Next quality step:** prove local persistence/indexing and integrate explicit Note/Loader exports.

See the ecosystem cockpit in [`constantin-jais/ecosystem/status.md`](https://github.com/constantin-jais/constantin-jais/blob/main/ecosystem/status.md).

## Dogfooding

This repository is part of the forge dogfooding loop: the ecosystem should use its own tools to make specs, maturity, contracts, releases, and product documentation observable.

Current visible evidence:

- contract and security workflows exercise memory/source/provenance contracts;
- README maturity notes keep local persistence and indexing limits explicit;
- fixtures describe the shape of trustworthy local context.

Expected next evidence:

- publish example context exports and code-map outputs;
- show Note and Loader handoff evidence through fixtures.

Dogfooding claims should stay backed by visible commands, fixtures, CI workflows, generated reports, or linked docs.

## Forge role

`gear-memory` is Gear infrastructure for trustworthy local context. It lets Rumble, Bolt, and Wrench share source references, memory entries, code maps, events, and provenance without each product inventing storage semantics.

## Boundary

It must not become an agent brain, product UX, raw ingestion engine, or artifact registry. Decisions stay in Bolt, experiences stay in Rumble, extraction stays in Wrench, and distribution stays in Gear Depot/Cable.

## Purpose

`gear-memory` is the persistent context substrate of the ecosystem. It provides code maps, repo memory, document/search primitives, and local-first context for agents and products.

It provides the ground truth that higher layers can query without owning the storage model.

## Owns

- Local-first memory and retrieval primitives.
- Code/document indexing and search substrate.
- Context persistence for agentic workflows.
- Clear interfaces for Bolt, Wrench, and Rumble consumers.
- `SourceRef`, `MemoryEntry`, `ProvenanceRecord`, `EventLogEntry`, and
  `CodeMap` substrate contracts.

## Does Not Own

- Agent decisions or workflow orchestration: belongs to Bolt.
- Product-specific UX: belongs to Rumble.
- Raw document extraction: belongs to Wrench.
- Artifact registry, distribution, or package policy: belongs to `gear-depot` / `gear-cable`.

## Allowed Dependencies

- Can ingest structured outputs produced by Wrench.
- Can be queried by Bolt and Rumble.
- Can rely on Gear-level storage/indexing primitives, preferably local and self-hostable.

## Product Vision Challenge

`gear-memory` must remain memory infrastructure, not an agent brain. Its success is measured by retrieval quality, determinism, locality, and inspectability.

## P0 Contracts

`gear-memory` currently exposes minimal serializable Rust contracts:

- `SourceRef`: stable reference to source material such as a file, URL, feed
  item, note block, transcript, document, dataset, or artifact reused as
  grounding input.
- `MemoryEntry`: indexable context record linked to a `SourceRef`.
- `ProvenanceRecord`: actor, operation, inputs, outputs, tool reference, and
  timestamp for traceability.
- `EventLogEntry`: append-only audit event shape with references and safe
  metadata.
- `CodeMap`: reproducible code/source symbol map; Wrench parses, Gear stores
  and indexes.
- `GearMemoryBundle`: `gear.memory.v0.1` contract family used by fixtures and
  the CLI validator.

These contracts deliberately do not model product semantics. Note blocks,
learning sessions, canvas specs, feed curation, and agent decisions remain owned
by Rumble or Bolt.

## Validation Rules

Validation is explicit through `validate()` on contract types.

- Required reference fields must be non-empty.
- SHA-256 hashes use `sha256:<64 hex chars>`.
- Timestamps use RFC3339 / ISO 8601 with an explicit offset, for example
  `2026-06-30T00:00:00Z`.
- Metadata rejects secret-like keys: `secret`, `token`, `password`,
  `credential`, and `api_key`.
- `Debug` output for metadata redacts values; callers must still validate before
  persistence.

## Example

```rust
use gear_memory::{
    SourceRef, SourceState, SourceType,
};

let source = SourceRef {
    source_id: "src_01".to_string(),
    source_type: SourceType::Document,
    origin_product: "wrench-loader".to_string(),
    uri: Some("file:///tmp/source.md".to_string()),
    content_hash: format!("sha256:{}", "a".repeat(64)),
    provenance_id: "prov_01".to_string(),
    state: SourceState::Active,
    created_at: "2026-06-30T00:00:00Z".to_string(),
};

source.validate()?;
```

Use `ProvenanceRecord::stable_hash()` when a deterministic audit identity is
needed for a serialized provenance record.

## CLI

```bash
cargo run -- validate tests/fixtures/gear-memory-minimal.valid.json
```

Expected output:

```text
valid gear-memory bundle: tests/fixtures/gear-memory-minimal.valid.json
```

P0 CLI validates contracts only. It does not persist, index, sync, or decide.
