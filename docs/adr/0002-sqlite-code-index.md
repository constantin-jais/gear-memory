# ADR-0002 — SQLite code index (P1) and external design references

- Status: Accepted
- Date: 2026-07-02
- Supersedes: the upstream-tracking section of ADR-0001

## Context

The ecosystem decomposition of `DeusData/codebase-memory-mcp`
(`constantin-jais/ecosystem/specs/shared/codebase-memory-mcp-decomposition.md`)
selected three of its elements for the first `gear-memory` indexing
increment: single-file SQLite+WAL storage (E4), deterministic structured
queries (E6), and precise source retrieval (E7). The progressive-indexing
ladder was amended the same day: rungs between catalog and vector are
commutative within one SQLite engine, so the graph rung may land before
full-text.

## Decision

1. **Engine: `rusqlite` (feature `bundled`).** Selected by a bounded
   micro-benchmark (annex below) with inspectability as the lexicographic
   first criterion: a `.sqlite3` file is readable by any standard tool,
   which is a stated success metric of this substrate.
2. **Schema:** canonical `record_json` per row for lossless contract
   roundtrips, plus extracted indexed columns; code maps are normalized
   into `code_symbols` / `code_edges` (replace semantics, one
   transaction). `PRAGMA user_version` tracks the schema; unknown
   versions are refused, never migrated silently.
3. **Query surface (deterministic by construction):** `symbol_search`
   (substring via `instr()`, no pattern metacharacters), `symbol_neighbors`
   (In/Out/Both), `trace_bfs` (bounded depth, lexicographic levels),
   `stats` (zero counts kept — a zero is a finding). No generic query
   language: the reference tool's own caveats (silent undercount at row
   caps) motivate named queries with explicit `truncated` metadata instead.
4. **Database location:** per-repo, git-ignored `./.gear-memory/db.sqlite3`
   by default, explicit `--db` for anything else, never a hidden global
   cache (locality and inspectability).
5. **External references, not upstreams:** `codebase-memory-mcp` is a
   design reference — no installation, no dependency, no code reuse.
   `basemind` (ADR-0001) is demoted from tracked upstream to knowledge
   reference: the crate is hand-written contract-first and no upstream
   sync ever happened. The sovereignty, licensing, and CI gates of
   ADR-0001 remain in force.
6. **Fixture-generator guardrail:** CodeMap test fixtures may be produced
   by a dev-only generator (`syn`, tests side, never shipped). The
   product keeps zero parsing capability — "Wrench parses, Gear stores".

## Benchmark annex (bounded, reproducible)

Workload: 10 000 symbols + 20 000 edges (deterministic LCG seed 42),
three query shapes, medians over 100 iterations after 10 warmups,
cross-engine result equality asserted. Source: `bench/storage-bench`
(standalone crate, excluded from this crate's dependency graph);
run with `cargo run --release` inside that directory. Apple M-series,
2026-07-02:

| engine                       | ingest (ms) | q1 search (µs) | q2 neighbors (µs) | q3 BFS≤3 (µs) | size (KiB) |
| ---------------------------- | ----------: | -------------: | ----------------: | ------------: | ---------: |
| rusqlite 0.40 (bundled, WAL) |        15.9 |          313.5 |               2.8 |          42.7 |      1 895 |
| redb 4.1 (pure Rust)         |        29.9 |          445.5 |               1.5 |          30.5 |      2 060 |
| stoolap 0.4 (pure Rust SQL)  |       106.0 |          470.7 |               1.8 |         406.5 |      5 969 |

Empirical findings that shaped the verdict:

- stoolap 0.4 only supports `INTEGER PRIMARY KEY` — every `gear-memory`
  contract keys on TEXT ids, so the natural schema is inexpressible;
  ingest is 6.7× slower and the footprint 3× larger.
- redb wins point lookups marginally but its format is unreadable outside
  Rust tooling, which fails the inspectability criterion outright.
- rusqlite: fastest ingest and scan, smallest footprint, and the file is
  inspectable with the ubiquitous `sqlite3` CLI.

## Alternatives rejected

- **redb / stoolap** — see annex.
- **sqlx** — async runtime for a local single-user store is dead weight.
- **openCypher subset** — deferred; named deterministic queries first,
  a query language only if real usage demands it.
- **Bundled embeddings / vector search** — last ladder rung by standing
  decision ("vector search must not become opaque truth").

## Licenses

`rusqlite` MIT, `libsqlite3-sys` MIT, SQLite public domain. The
transitive `foldhash` (via `hashlink`/`hashbrown`) is Zlib — permissive,
OSI/FSF approved — added to the `deny.toml` allow list by this ADR.
`cargo deny check licenses` green. Dev-only: `syn`, `proc-macro2`
(MIT/Apache-2.0).
