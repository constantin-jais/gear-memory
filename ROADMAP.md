# Roadmap

This is a contribution map, not a startup roadmap or a delivery promise. It shows where help is useful while keeping scope explicit.

## Now

- make dogfooding evidence visible through commands, fixtures, CI checks, generated reports, or linked docs;
- stabilize memory/source/provenance fixtures;
- document privacy and local persistence limits;
- improve examples for Note and Loader handoff;
- keep CI, contracts, and security checks green.

## Next

- improve error messages around invalid references;
- bundle export/import commands (artifact custody stays `gear-depot`);
- full-text rung (FTS5) inside the same SQLite engine;
- integrate explicit Note/Loader exports.

## Done in P1 (2026-07-02)

- contract tests for local persistence and indexing (shared suite,
  `FileStore` + `SqliteStore`);
- first alpha-quality memory substrate: SQLite code index with
  deterministic graph queries, CLI envelopes, and dogfooding evidence
  (`docs/dogfooding.md`, ADR-0002).

## Later

- broader local integrations;
- provenance for shared memory artifacts;
- contracts v0.2 when a real producer emits new shapes (see the
  ecosystem decision log, 2026-07-02);
- multi-user memory only when privacy and tenancy boundaries are explicit.
