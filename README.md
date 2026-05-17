# Gear Memory

**Layer:** Gear — Infrastructure  
**Role:** local context and memory substrate  
**Mission:** provide sovereign, inspectable primitives for storing, indexing, searching, and retrieving project or agent context.

---

## Purpose

`gear-memory` is the persistent context substrate of the ecosystem. It provides code maps, repo memory, document/search primitives, and local-first context for agents and products.

It provides the ground truth that higher layers can query without owning the storage model.

## Owns

- Local-first memory and retrieval primitives.
- Code/document indexing and search substrate.
- Context persistence for agentic workflows.
- Clear interfaces for Bolt, Wrench, and Rumble consumers.

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
