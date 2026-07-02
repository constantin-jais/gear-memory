//! SqliteStore-specific index queries: the P1 query surface
//! (symbol search, neighbors, bounded BFS, stats, bundle ingestion).
//!
//! These are inherent `SqliteStore` capabilities — `FileStore` is the
//! unindexed baseline and deliberately does not grow them.
use tempfile::TempDir;

use gear_memory::{
    CodeEdge, CodeEdgeKind, CodeMap, CodeMapScope, CodeMapState, CodeSymbol, CodeSymbolKind,
    EdgeDirection, GearMemoryBundle, ProvenanceOperation, ProvenanceRecord, SafeMetadata,
    SourceRange, SourceRef, SourceState, SourceType, SqliteStore, Store,
};

fn hash() -> String {
    format!("sha256:{}", "a".repeat(64))
}

fn store() -> (SqliteStore, TempDir) {
    let dir = TempDir::new().expect("create temp dir");
    let store = SqliteStore::new(&dir.path().join("gear.sqlite3")).expect("create sqlite store");
    (store, dir)
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

fn edge(from: &str, to: &str, kind: CodeEdgeKind) -> CodeEdge {
    CodeEdge {
        from: from.to_string(),
        to: to.to_string(),
        kind,
    }
}

/// alpha --calls--> beta --calls--> gamma ; test_alpha --tests--> alpha ;
/// one `Type` symbol without edges.
fn demo_code_map() -> CodeMap {
    CodeMap {
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
            symbol("sym_gamma", "gear::gamma", CodeSymbolKind::Function),
            symbol("sym_test_alpha", "gear::test_alpha", CodeSymbolKind::Test),
            symbol("sym_spec", "gear::SpecShape", CodeSymbolKind::Type),
        ],
        edges: vec![
            edge("sym_alpha", "sym_beta", CodeEdgeKind::Calls),
            edge("sym_beta", "sym_gamma", CodeEdgeKind::Calls),
            edge("sym_test_alpha", "sym_alpha", CodeEdgeKind::Tests),
        ],
        state: CodeMapState::Active,
        created_at: "2026-06-30T00:00:00Z".to_string(),
    }
}

fn valid_source_ref(source_id: &str) -> SourceRef {
    SourceRef {
        source_id: source_id.to_string(),
        source_type: SourceType::File,
        origin_product: "test-loader".to_string(),
        uri: Some("file:///tmp/lib.rs".to_string()),
        content_hash: hash(),
        provenance_id: "prov_src".to_string(),
        state: SourceState::Active,
        created_at: "2026-06-30T00:00:00Z".to_string(),
        metadata: SafeMetadata::default(),
    }
}

fn ingest_provenance(provenance_id: &str) -> ProvenanceRecord {
    ProvenanceRecord {
        provenance_id: provenance_id.to_string(),
        actor_ref: "gear-memory:test".to_string(),
        operation: ProvenanceOperation::Indexed,
        inputs: vec!["bundle.json".to_string()],
        outputs: vec!["cm_demo".to_string()],
        tool_ref: Some("gear-memory@test".to_string()),
        timestamp: "2026-07-02T00:00:00Z".to_string(),
        metadata: SafeMetadata::default(),
    }
}

fn demo_bundle() -> GearMemoryBundle {
    GearMemoryBundle {
        format: "gear.memory.v0.1".to_string(),
        source_refs: vec![valid_source_ref("src_01")],
        memory_entries: vec![],
        event_log_entries: vec![],
        code_maps: vec![demo_code_map()],
        provenance_records: vec![],
    }
}

#[test]
fn symbol_search_orders_by_name_then_id() {
    let (store, _guard) = store();
    store.put_code_map(&demo_code_map()).expect("stored");

    let hits = store
        .symbol_search("gear::", None)
        .expect("search succeeds");
    let names: Vec<&str> = hits.iter().map(|(_, s)| s.name.as_str()).collect();

    assert_eq!(
        names,
        vec![
            "gear::SpecShape",
            "gear::alpha",
            "gear::beta",
            "gear::gamma",
            "gear::test_alpha",
        ]
    );
}

#[test]
fn symbol_search_filters_by_kind() {
    let (store, _guard) = store();
    store.put_code_map(&demo_code_map()).expect("stored");

    let hits = store
        .symbol_search("gear::", Some(&CodeSymbolKind::Type))
        .expect("search succeeds");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].1.name, "gear::SpecShape");
}

#[test]
fn symbol_search_is_deterministic() {
    let (store, _guard) = store();
    store.put_code_map(&demo_code_map()).expect("stored");

    let first = store.symbol_search("a", None).expect("first run");
    let second = store.symbol_search("a", None).expect("second run");

    assert_eq!(first, second);
}

#[test]
fn symbol_neighbors_directional() {
    let (store, _guard) = store();
    store.put_code_map(&demo_code_map()).expect("stored");

    let out = store
        .symbol_neighbors("cm_demo", "sym_alpha", EdgeDirection::Out, None)
        .expect("out neighbors");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].to, "sym_beta");

    let inbound = store
        .symbol_neighbors("cm_demo", "sym_beta", EdgeDirection::In, None)
        .expect("in neighbors");
    assert_eq!(inbound.len(), 1);
    assert_eq!(inbound[0].from, "sym_alpha");

    let both = store
        .symbol_neighbors("cm_demo", "sym_beta", EdgeDirection::Both, None)
        .expect("both neighbors");
    assert_eq!(both.len(), 2);
}

#[test]
fn symbol_neighbors_filters_by_edge_kind() {
    let (store, _guard) = store();
    store.put_code_map(&demo_code_map()).expect("stored");

    let tests_only = store
        .symbol_neighbors(
            "cm_demo",
            "sym_alpha",
            EdgeDirection::In,
            Some(&CodeEdgeKind::Tests),
        )
        .expect("in neighbors");

    assert_eq!(tests_only.len(), 1);
    assert_eq!(tests_only[0].from, "sym_test_alpha");
}

#[test]
fn trace_bfs_is_bounded_and_ordered() {
    let (store, _guard) = store();
    store.put_code_map(&demo_code_map()).expect("stored");

    let shallow = store
        .trace_bfs("cm_demo", "sym_alpha", 1)
        .expect("bfs depth 1");
    let shallow_ids: Vec<(u32, &str)> = shallow
        .iter()
        .map(|hop| (hop.depth, hop.symbol_id.as_str()))
        .collect();
    assert_eq!(shallow_ids, vec![(0, "sym_alpha"), (1, "sym_beta")]);

    let deep = store
        .trace_bfs("cm_demo", "sym_alpha", 3)
        .expect("bfs depth 3");
    let deep_ids: Vec<(u32, &str)> = deep
        .iter()
        .map(|hop| (hop.depth, hop.symbol_id.as_str()))
        .collect();
    assert_eq!(
        deep_ids,
        vec![(0, "sym_alpha"), (1, "sym_beta"), (2, "sym_gamma")]
    );
}

#[test]
fn stats_keeps_zero_counts() {
    let (store, _guard) = store();

    let empty = store.stats().expect("stats on empty store");
    assert_eq!(empty.schema_version, 1);
    assert_eq!(empty.entities.get("code_maps"), Some(&0));
    assert_eq!(empty.symbols_by_kind.get("function"), Some(&0));
    assert_eq!(empty.edges_by_kind.get("supersedes"), Some(&0));

    store.put_code_map(&demo_code_map()).expect("stored");

    let filled = store.stats().expect("stats after put");
    assert_eq!(filled.entities.get("code_maps"), Some(&1));
    assert_eq!(filled.entities.get("code_symbols"), Some(&5));
    assert_eq!(filled.entities.get("code_edges"), Some(&3));
    assert_eq!(filled.symbols_by_kind.get("function"), Some(&3));
    assert_eq!(filled.symbols_by_kind.get("test"), Some(&1));
    assert_eq!(filled.symbols_by_kind.get("type"), Some(&1));
    assert_eq!(filled.symbols_by_kind.get("route"), Some(&0));
    assert_eq!(filled.edges_by_kind.get("calls"), Some(&2));
    assert_eq!(filled.edges_by_kind.get("tests"), Some(&1));
    assert_eq!(filled.edges_by_kind.get("imports"), Some(&0));
}

#[test]
fn ingest_bundle_reports_counts_and_writes_provenance() {
    let (store, _guard) = store();

    let report = store
        .ingest_bundle(&demo_bundle(), &ingest_provenance("prov_ingest_01"))
        .expect("ingest succeeds");

    assert_eq!(report.source_refs, 1);
    assert_eq!(report.code_maps, 1);
    assert_eq!(report.code_symbols, 5);
    assert_eq!(report.code_edges, 3);

    let provenance = store
        .get_provenance_record("prov_ingest_01")
        .expect("get succeeds")
        .expect("ingest provenance persisted");
    assert_eq!(provenance.operation, ProvenanceOperation::Indexed);

    assert!(
        store
            .get_source_ref("src_01")
            .expect("get succeeds")
            .is_some()
    );
}

#[test]
fn ingest_bundle_twice_is_idempotent() {
    let (store, _guard) = store();

    store
        .ingest_bundle(&demo_bundle(), &ingest_provenance("prov_ingest_01"))
        .expect("first ingest");
    let first = store.stats().expect("stats after first ingest");

    store
        .ingest_bundle(&demo_bundle(), &ingest_provenance("prov_ingest_01"))
        .expect("second ingest");
    let second = store.stats().expect("stats after second ingest");

    assert_eq!(first, second);
}
