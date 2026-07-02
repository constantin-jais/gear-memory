//! Dogfooding evidence: the committed CodeMap fixture of the gear-memory
//! repository itself is valid, realistic, and queryable end to end through
//! the CLI with byte-identical (deterministic) output.
use std::process::Command;

use tempfile::TempDir;

use gear_memory::{CodeSymbolKind, GearMemoryBundle};

const FIXTURE_PATH: &str = "tests/fixtures/gear-memory-repo-codemap.valid.json";

fn fixture_bundle() -> GearMemoryBundle {
    let payload = std::fs::read_to_string(FIXTURE_PATH).expect("read committed fixture");
    serde_json::from_str(&payload).expect("fixture parses")
}

#[test]
fn repo_codemap_fixture_is_valid_and_realistic() {
    let bundle = fixture_bundle();
    bundle.validate().expect("fixture bundle validates");

    let code_map = &bundle.code_maps[0];
    assert!(
        code_map.symbols.len() >= 50,
        "expected a realistic symbol count, got {}",
        code_map.symbols.len()
    );
    assert!(
        !code_map.edges.is_empty(),
        "expected at least one call edge"
    );

    let has_sqlite_store_type = code_map.symbols.iter().any(|symbol| {
        symbol.kind == CodeSymbolKind::Type && symbol.name.ends_with("::SqliteStore")
    });
    assert!(has_sqlite_store_type, "SqliteStore type is mapped");

    let has_store_trait = code_map
        .symbols
        .iter()
        .any(|symbol| symbol.name.ends_with("lib::Store"));
    assert!(has_store_trait, "Store trait is mapped");
}

#[test]
fn cli_queries_over_repo_fixture_are_deterministic() {
    let guard = TempDir::new().expect("create temp dir");
    let db_path = guard.path().join("gear.sqlite3");
    let db = db_path.to_string_lossy();

    let ingest = Command::new(env!("CARGO_BIN_EXE_gear-memory"))
        .args(["ingest", FIXTURE_PATH, "--db", &db])
        .output()
        .expect("run ingest");
    assert!(
        ingest.status.success(),
        "ingest failed: {}",
        String::from_utf8_lossy(&ingest.stderr)
    );

    let run_query = || {
        let output = Command::new(env!("CARGO_BIN_EXE_gear-memory"))
            .args(["query", "symbols", "--name", "Store", "--db", &db])
            .output()
            .expect("run query");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    };

    let first = run_query();
    let second = run_query();
    assert_eq!(first, second, "same query must produce identical bytes");

    let envelope: serde_json::Value = serde_json::from_str(&first).expect("query envelope");
    assert!(
        envelope["meta"]["count"].as_u64().unwrap_or(0) > 0,
        "querying `Store` over the repo fixture returns symbols"
    );
}
