//! End-to-end CLI tests for the SQLite index commands
//! (`ingest`, `query symbols`, `trace`, `stats`): `{ data, meta }`
//! envelope, deterministic output, idempotent re-ingestion.
use std::process::Command;

use tempfile::TempDir;

use gear_memory::{
    CodeEdge, CodeEdgeKind, CodeMap, CodeMapScope, CodeMapState, CodeSymbol, CodeSymbolKind,
    GearMemoryBundle, SafeMetadata, SourceRange, SourceRef, SourceState, SourceType,
};

fn hash() -> String {
    format!("sha256:{}", "a".repeat(64))
}

fn symbol(symbol_id: &str, name: &str, kind: CodeSymbolKind) -> CodeSymbol {
    CodeSymbol {
        symbol_id: symbol_id.to_string(),
        kind,
        name: name.to_string(),
        source_ref: "src_01".to_string(),
        range: SourceRange {
            start_line: 1,
            end_line: 3,
        },
        content_hash: hash(),
    }
}

fn demo_bundle() -> GearMemoryBundle {
    GearMemoryBundle {
        format: "gear.memory.v0.1".to_string(),
        source_refs: vec![SourceRef {
            source_id: "src_01".to_string(),
            source_type: SourceType::File,
            origin_product: "test-loader".to_string(),
            uri: Some("file:///tmp/lib.rs".to_string()),
            content_hash: hash(),
            provenance_id: "prov_src".to_string(),
            state: SourceState::Active,
            created_at: "2026-06-30T00:00:00Z".to_string(),
            metadata: SafeMetadata::default(),
        }],
        memory_entries: vec![],
        event_log_entries: vec![],
        code_maps: vec![CodeMap {
            code_map_id: "cm_demo".to_string(),
            root_source_ref: "src_01".to_string(),
            scope: CodeMapScope {
                repo_ref: Some("repo_demo".to_string()),
                revision: "git:abc123".to_string(),
                paths: vec!["src/".to_string()],
            },
            parser_refs: vec!["test-fixture:handwritten".to_string()],
            symbols: vec![
                symbol("sym_alpha", "gear::alpha", CodeSymbolKind::Function),
                symbol("sym_beta", "gear::beta", CodeSymbolKind::Function),
            ],
            edges: vec![CodeEdge {
                from: "sym_alpha".to_string(),
                to: "sym_beta".to_string(),
                kind: CodeEdgeKind::Calls,
            }],
            state: CodeMapState::Active,
            created_at: "2026-06-30T00:00:00Z".to_string(),
        }],
        provenance_records: vec![],
    }
}

struct CliContext {
    _guard: TempDir,
    bundle_path: String,
    db_path: String,
}

fn context() -> CliContext {
    let guard = TempDir::new().expect("create temp dir");
    let bundle_path = guard.path().join("bundle.json");
    let db_path = guard.path().join("gear.sqlite3");

    let payload = serde_json::to_string_pretty(&demo_bundle()).expect("serialize bundle");
    std::fs::write(&bundle_path, payload).expect("write bundle file");

    CliContext {
        bundle_path: bundle_path.to_string_lossy().into_owned(),
        db_path: db_path.to_string_lossy().into_owned(),
        _guard: guard,
    }
}

fn run(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_gear-memory"))
        .args(args)
        .output()
        .expect("run gear-memory");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn parsed(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout).expect("stdout is a JSON envelope")
}

#[test]
fn cli_ingest_reports_counts_in_envelope() {
    let ctx = context();

    let (ok, stdout, stderr) = run(&["ingest", &ctx.bundle_path, "--db", &ctx.db_path]);
    assert!(ok, "ingest failed: {stderr}");

    let envelope = parsed(&stdout);
    assert_eq!(envelope["data"]["report"]["code_maps"], 1);
    assert_eq!(envelope["data"]["report"]["code_symbols"], 2);
    assert_eq!(envelope["data"]["report"]["code_edges"], 1);
    assert_eq!(envelope["meta"]["api"], "gear.memory.v0.1");
}

#[test]
fn cli_ingest_twice_is_idempotent() {
    let ctx = context();

    run(&["ingest", &ctx.bundle_path, "--db", &ctx.db_path]);
    let (_, first_stats, _) = run(&["stats", "--db", &ctx.db_path]);

    run(&["ingest", &ctx.bundle_path, "--db", &ctx.db_path]);
    let (_, second_stats, _) = run(&["stats", "--db", &ctx.db_path]);

    assert_eq!(first_stats, second_stats);
}

#[test]
fn cli_query_symbols_is_deterministic() {
    let ctx = context();
    run(&["ingest", &ctx.bundle_path, "--db", &ctx.db_path]);

    let (ok, first, stderr) = run(&["query", "symbols", "--name", "gear::", "--db", &ctx.db_path]);
    assert!(ok, "query failed: {stderr}");
    let (_, second, _) = run(&["query", "symbols", "--name", "gear::", "--db", &ctx.db_path]);

    assert_eq!(first, second, "same query must produce identical bytes");

    let envelope = parsed(&first);
    assert_eq!(envelope["meta"]["count"], 2);
    assert_eq!(
        envelope["data"]["symbols"][0]["symbol"]["name"],
        "gear::alpha"
    );
}

#[test]
fn cli_query_symbols_filters_by_kind() {
    let ctx = context();
    run(&["ingest", &ctx.bundle_path, "--db", &ctx.db_path]);

    let (ok, stdout, stderr) = run(&[
        "query",
        "symbols",
        "--name",
        "gear::",
        "--kind",
        "function",
        "--db",
        &ctx.db_path,
    ]);
    assert!(ok, "query failed: {stderr}");
    assert_eq!(parsed(&stdout)["meta"]["count"], 2);

    let (ok, stdout, _) = run(&[
        "query",
        "symbols",
        "--name",
        "gear::",
        "--kind",
        "test",
        "--db",
        &ctx.db_path,
    ]);
    assert!(ok);
    assert_eq!(parsed(&stdout)["meta"]["count"], 0);
}

#[test]
fn cli_trace_lists_hops_in_depth_order() {
    let ctx = context();
    run(&["ingest", &ctx.bundle_path, "--db", &ctx.db_path]);

    let (ok, stdout, stderr) = run(&[
        "trace",
        "cm_demo",
        "sym_alpha",
        "--depth",
        "3",
        "--db",
        &ctx.db_path,
    ]);
    assert!(ok, "trace failed: {stderr}");

    let envelope = parsed(&stdout);
    let hops = envelope["data"]["hops"]
        .as_array()
        .expect("hops is an array");
    assert_eq!(hops.len(), 2);
    assert_eq!(hops[0]["depth"], 0);
    assert_eq!(hops[0]["symbol_id"], "sym_alpha");
    assert_eq!(hops[1]["depth"], 1);
    assert_eq!(hops[1]["symbol_id"], "sym_beta");
}

#[test]
fn cli_stats_keeps_zero_counts() {
    let ctx = context();
    run(&["ingest", &ctx.bundle_path, "--db", &ctx.db_path]);

    let (ok, stdout, stderr) = run(&["stats", "--db", &ctx.db_path]);
    assert!(ok, "stats failed: {stderr}");

    let envelope = parsed(&stdout);
    assert_eq!(envelope["data"]["schema_version"], 1);
    assert_eq!(envelope["data"]["entities"]["code_maps"], 1);
    assert_eq!(envelope["data"]["symbols_by_kind"]["function"], 2);
    assert_eq!(envelope["data"]["symbols_by_kind"]["route"], 0);
    assert_eq!(envelope["data"]["edges_by_kind"]["calls"], 1);
    assert_eq!(envelope["data"]["edges_by_kind"]["supersedes"], 0);
}
