//! Contract suite shared by every `Store` backend.
//!
//! Each check is a plain function over `&dyn Store`; `contract_tests!`
//! instantiates the full set once per backend (`FileStore`, `SqliteStore`),
//! so no backend can ship with divergent persistence or erasure semantics.
//! Supersedes the Stage 0 `stage0_store_and_lookups.rs` single-backend suite.
use tempfile::TempDir;

use gear_memory::{
    CodeEdge, CodeEdgeKind, CodeMap, CodeMapScope, CodeMapState, CodeSymbol, CodeSymbolKind,
    EventLogEntry, FileStore, IndexMetadata, IndexState, MemoryEntry, ProvenanceOperation,
    ProvenanceRecord, SafeMetadata, SourceRange, SourceRef, SourceState, SourceType, SqliteStore,
    Store,
};

fn hash() -> String {
    format!("sha256:{}", "a".repeat(64))
}

fn valid_source_ref(source_id: &str) -> SourceRef {
    SourceRef {
        source_id: source_id.to_string(),
        source_type: SourceType::Document,
        origin_product: "test-loader".to_string(),
        uri: Some("file:///tmp/test.md".to_string()),
        content_hash: hash(),
        provenance_id: "prov_01".to_string(),
        state: SourceState::Active,
        created_at: "2026-06-30T00:00:00Z".to_string(),
        metadata: SafeMetadata::default(),
    }
}

fn valid_memory_entry(memory_entry_id: &str, source_id: &str) -> MemoryEntry {
    MemoryEntry {
        memory_entry_id: memory_entry_id.to_string(),
        source_ref: source_id.to_string(),
        content_hash: hash(),
        index_state: IndexState::Indexed,
        index_metadata: IndexMetadata {
            schema_version: "memory-entry.v0.1".to_string(),
            chunk_count: 1,
            embedding_model_ref: None,
            indexed_at: Some("2026-06-30T00:01:00Z".to_string()),
        },
        created_at: "2026-06-30T00:00:00Z".to_string(),
    }
}

fn valid_provenance_record(provenance_id: &str, source_id: &str) -> ProvenanceRecord {
    ProvenanceRecord {
        provenance_id: provenance_id.to_string(),
        actor_ref: "test-actor".to_string(),
        operation: ProvenanceOperation::Created,
        inputs: vec![],
        outputs: vec![source_id.to_string()],
        tool_ref: Some("test-tool".to_string()),
        timestamp: "2026-06-30T00:00:00Z".to_string(),
        metadata: SafeMetadata::default(),
    }
}

fn valid_event_log_entry(event_id: &str, target_ref: &str, provenance_id: &str) -> EventLogEntry {
    EventLogEntry {
        event_id: event_id.to_string(),
        event_type: "source.created".to_string(),
        actor_ref: "test-actor".to_string(),
        target_ref: target_ref.to_string(),
        provenance_id: provenance_id.to_string(),
        metadata: SafeMetadata::default(),
        created_at: "2026-06-30T00:00:00Z".to_string(),
    }
}

fn valid_code_map(code_map_id: &str) -> CodeMap {
    CodeMap {
        code_map_id: code_map_id.to_string(),
        root_source_ref: "src_01".to_string(),
        scope: CodeMapScope {
            repo_ref: Some("repo_demo".to_string()),
            revision: "git:abc123".to_string(),
            paths: vec!["src/".to_string()],
        },
        parser_refs: vec!["tree-sitter:rust@0.0.0-demo".to_string()],
        symbols: vec![
            CodeSymbol {
                symbol_id: "sym_main".to_string(),
                kind: CodeSymbolKind::Function,
                name: "demo::main".to_string(),
                source_ref: "src_01".to_string(),
                range: SourceRange {
                    start_line: 1,
                    end_line: 3,
                },
                content_hash: hash(),
            },
            CodeSymbol {
                symbol_id: "sym_helper".to_string(),
                kind: CodeSymbolKind::Function,
                name: "demo::helper".to_string(),
                source_ref: "src_01".to_string(),
                range: SourceRange {
                    start_line: 5,
                    end_line: 9,
                },
                content_hash: hash(),
            },
        ],
        edges: vec![CodeEdge {
            from: "sym_main".to_string(),
            to: "sym_helper".to_string(),
            kind: CodeEdgeKind::Calls,
        }],
        state: CodeMapState::Active,
        created_at: "2026-06-30T00:00:00Z".to_string(),
    }
}

mod suite {
    use super::*;

    pub fn roundtrip_source_ref(store: &dyn Store) {
        let source = valid_source_ref("src_01");
        source.validate().expect("source is valid");

        store.put_source_ref(&source).expect("source is stored");

        let retrieved = store
            .get_source_ref("src_01")
            .expect("get succeeds")
            .expect("source exists");

        assert_eq!(retrieved, source);
    }

    pub fn roundtrip_memory_entry(store: &dyn Store) {
        let entry = valid_memory_entry("mem_01", "src_01");
        entry.validate().expect("entry is valid");

        store.put_memory_entry(&entry).expect("entry is stored");

        let retrieved = store
            .get_memory_entry("mem_01")
            .expect("get succeeds")
            .expect("entry exists");

        assert_eq!(retrieved, entry);
    }

    pub fn roundtrip_provenance_record(store: &dyn Store) {
        let record = valid_provenance_record("prov_01", "src_01");
        record.validate().expect("record is valid");

        store
            .put_provenance_record(&record)
            .expect("record is stored");

        let retrieved = store
            .get_provenance_record("prov_01")
            .expect("get succeeds")
            .expect("record exists");

        assert_eq!(retrieved, record);
    }

    pub fn roundtrip_event_log_entry(store: &dyn Store) {
        let event = valid_event_log_entry("evt_01", "src_01", "prov_01");
        event.validate().expect("event is valid");

        store.put_event_log_entry(&event).expect("event is stored");

        let retrieved = store
            .get_event_log_entry("evt_01")
            .expect("get succeeds")
            .expect("event exists");

        assert_eq!(retrieved, event);
    }

    pub fn roundtrip_code_map(store: &dyn Store) {
        let code_map = valid_code_map("cm_01");
        code_map.validate().expect("code map is valid");

        store.put_code_map(&code_map).expect("code map is stored");

        let retrieved = store
            .get_code_map("cm_01")
            .expect("get succeeds")
            .expect("code map exists");

        assert_eq!(retrieved, code_map);
    }

    pub fn code_map_reput_replaces_content(store: &dyn Store) {
        let mut code_map = valid_code_map("cm_01");
        store.put_code_map(&code_map).expect("first put");

        code_map.symbols.truncate(1);
        code_map.edges.clear();
        store.put_code_map(&code_map).expect("second put");

        let retrieved = store
            .get_code_map("cm_01")
            .expect("get succeeds")
            .expect("code map exists");

        assert_eq!(retrieved.symbols.len(), 1);
        assert!(retrieved.edges.is_empty());
    }

    pub fn put_rejects_invalid_source_ref(store: &dyn Store) {
        let mut source = valid_source_ref("src_01");
        source.content_hash = "sha256:not-hex".to_string();

        store
            .put_source_ref(&source)
            .expect_err("invalid contract is rejected before persistence");

        assert!(
            store
                .get_source_ref("src_01")
                .expect("get succeeds")
                .is_none(),
            "nothing was persisted"
        );
    }

    pub fn lookup_by_id_returns_existing_source_ref(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored");

        let found = store
            .lookup_source_refs_by_id("src_01")
            .expect("lookup succeeds");

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source_id, "src_01");
    }

    pub fn lookup_by_id_returns_none_for_missing_source_ref(store: &dyn Store) {
        let found = store
            .lookup_source_refs_by_id("nonexistent")
            .expect("lookup succeeds");

        assert_eq!(found.len(), 0);
    }

    pub fn lookup_by_content_hash_finds_source_ref(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored");

        let found = store
            .lookup_source_refs_by_content_hash(&hash())
            .expect("lookup succeeds");

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source_id, "src_01");
    }

    pub fn lookup_by_origin_product_filters_correctly(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored src_01");

        let mut other = valid_source_ref("src_02");
        other.origin_product = "other-loader".to_string();
        store.put_source_ref(&other).expect("stored src_02");

        let found = store
            .lookup_source_refs_by_origin_product("test-loader")
            .expect("lookup succeeds");

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source_id, "src_01");
    }

    pub fn lookup_by_state_filters_correctly(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored src_01");

        let mut deleted = valid_source_ref("src_02");
        deleted.state = SourceState::Deleted;
        store.put_source_ref(&deleted).expect("stored src_02");

        let active = store
            .lookup_source_refs_by_state(&SourceState::Active)
            .expect("lookup active succeeds");
        let deleted = store
            .lookup_source_refs_by_state(&SourceState::Deleted)
            .expect("lookup deleted succeeds");

        assert_eq!(active.len(), 1);
        assert_eq!(deleted.len(), 1);
    }

    pub fn lookup_by_timestamp_range_filters_correctly(store: &dyn Store) {
        let mut early = valid_source_ref("src_01");
        early.created_at = "2026-06-30T00:00:00Z".to_string();
        store.put_source_ref(&early).expect("stored");

        let mut late = valid_source_ref("src_02");
        late.created_at = "2026-07-01T00:00:00Z".to_string();
        store.put_source_ref(&late).expect("stored");

        let in_range = store
            .lookup_source_refs_by_timestamp_range("2026-06-29T00:00:00Z", "2026-06-30T12:00:00Z")
            .expect("lookup succeeds");

        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].source_id, "src_01");
    }

    pub fn lookup_memory_entries_by_state_filters_correctly(store: &dyn Store) {
        store
            .put_memory_entry(&valid_memory_entry("mem_01", "src_01"))
            .expect("stored mem_01");

        let mut pending = valid_memory_entry("mem_02", "src_01");
        pending.index_state = IndexState::Pending;
        store.put_memory_entry(&pending).expect("stored mem_02");

        let indexed = store
            .lookup_memory_entries_by_state(&IndexState::Indexed)
            .expect("lookup succeeds");

        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].memory_entry_id, "mem_01");
    }

    pub fn mark_deleted_transitions_state_and_creates_provenance(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored");

        store
            .mark_deleted("src_01", "GDPR deletion request", "2026-07-01T00:00:00Z")
            .expect("mark deleted succeeds");

        let retrieved = store
            .get_source_ref("src_01")
            .expect("get succeeds")
            .expect("source still exists");
        assert_eq!(retrieved.state, SourceState::Deleted);

        let deletion_records: Vec<_> = store
            .list_all_provenance_records()
            .expect("list provenances succeeds")
            .into_iter()
            .filter(|r| r.operation == ProvenanceOperation::Deleted)
            .collect();

        assert!(!deletion_records.is_empty());
    }

    pub fn mark_deleted_rejects_double_deletion(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored");

        store
            .mark_deleted("src_01", "first deletion", "2026-07-01T00:00:00Z")
            .expect("first deletion succeeds");

        let error = store
            .mark_deleted("src_01", "second deletion attempt", "2026-07-01T00:00:00Z")
            .expect_err("double deletion should fail");

        assert!(error.to_string().contains("already deleted"));
    }

    pub fn mark_anonymized_transitions_state_and_creates_provenance(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored");

        store
            .mark_anonymized("src_01", "GDPR anonymization", "2026-07-01T00:00:00Z")
            .expect("mark anonymized succeeds");

        let retrieved = store
            .get_source_ref("src_01")
            .expect("get succeeds")
            .expect("source still exists");
        assert_eq!(retrieved.state, SourceState::Anonymized);

        let anon_records: Vec<_> = store
            .list_all_provenance_records()
            .expect("list provenances succeeds")
            .into_iter()
            .filter(|r| r.operation == ProvenanceOperation::Anonymized)
            .collect();

        assert!(!anon_records.is_empty());
    }

    pub fn lookup_excludes_deleted_from_active_searches(store: &dyn Store) {
        store
            .put_source_ref(&valid_source_ref("src_01"))
            .expect("stored");

        store
            .mark_deleted("src_01", "removed", "2026-07-01T00:00:00Z")
            .expect("deleted");

        let active = store
            .lookup_source_refs_by_state(&SourceState::Active)
            .expect("lookup succeeds");
        assert_eq!(active.len(), 0);

        let deleted = store
            .lookup_source_refs_by_state(&SourceState::Deleted)
            .expect("lookup succeeds");
        assert_eq!(deleted.len(), 1);
    }
}

fn file_store() -> (Box<dyn Store>, TempDir) {
    let dir = TempDir::new().expect("create temp dir");
    let store = FileStore::new(dir.path()).expect("create file store");
    (Box::new(store), dir)
}

fn sqlite_store() -> (Box<dyn Store>, TempDir) {
    let dir = TempDir::new().expect("create temp dir");
    let store = SqliteStore::new(&dir.path().join("gear.sqlite3")).expect("create sqlite store");
    (Box::new(store), dir)
}

macro_rules! contract_tests {
    ($backend:ident) => {
        mod $backend {
            macro_rules! backend_test {
                ($name:ident) => {
                    #[test]
                    fn $name() {
                        let (store, _guard) = super::$backend();
                        super::suite::$name(store.as_ref());
                    }
                };
            }

            backend_test!(roundtrip_source_ref);
            backend_test!(roundtrip_memory_entry);
            backend_test!(roundtrip_provenance_record);
            backend_test!(roundtrip_event_log_entry);
            backend_test!(roundtrip_code_map);
            backend_test!(code_map_reput_replaces_content);
            backend_test!(put_rejects_invalid_source_ref);
            backend_test!(lookup_by_id_returns_existing_source_ref);
            backend_test!(lookup_by_id_returns_none_for_missing_source_ref);
            backend_test!(lookup_by_content_hash_finds_source_ref);
            backend_test!(lookup_by_origin_product_filters_correctly);
            backend_test!(lookup_by_state_filters_correctly);
            backend_test!(lookup_by_timestamp_range_filters_correctly);
            backend_test!(lookup_memory_entries_by_state_filters_correctly);
            backend_test!(mark_deleted_transitions_state_and_creates_provenance);
            backend_test!(mark_deleted_rejects_double_deletion);
            backend_test!(mark_anonymized_transitions_state_and_creates_provenance);
            backend_test!(lookup_excludes_deleted_from_active_searches);
        }
    };
}

contract_tests!(file_store);
contract_tests!(sqlite_store);
