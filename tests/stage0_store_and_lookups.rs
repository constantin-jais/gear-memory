/// Stage 0 store and lookup tests.
///
/// Tests RED-first: store persistence, round-trip integrity, lookups, and RGPD erasure.
use tempfile::TempDir;

use gear_memory::{
    EventLogEntry, IndexMetadata, IndexState, MemoryEntry, ProvenanceOperation, ProvenanceRecord,
    SafeMetadata, SourceRef, SourceState, SourceType, Store,
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

#[test]
fn store_roundtrip_source_ref_preserves_data() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source = valid_source_ref("src_01");
    source.validate().expect("source is valid");

    store.put_source_ref(&source).expect("source is stored");

    let retrieved = store
        .get_source_ref("src_01")
        .expect("get succeeds")
        .expect("source exists");

    assert_eq!(retrieved.source_id, source.source_id);
    assert_eq!(retrieved.content_hash, source.content_hash);
    assert_eq!(retrieved.state, source.state);
    assert_eq!(retrieved.created_at, source.created_at);
}

#[test]
fn store_roundtrip_memory_entry_preserves_data() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let entry = valid_memory_entry("mem_01", "src_01");
    entry.validate().expect("entry is valid");

    store.put_memory_entry(&entry).expect("entry is stored");

    let retrieved = store
        .get_memory_entry("mem_01")
        .expect("get succeeds")
        .expect("entry exists");

    assert_eq!(retrieved.memory_entry_id, entry.memory_entry_id);
    assert_eq!(retrieved.source_ref, entry.source_ref);
    assert_eq!(retrieved.content_hash, entry.content_hash);
}

#[test]
fn store_roundtrip_provenance_record_preserves_data() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let record = valid_provenance_record("prov_01", "src_01");
    record.validate().expect("record is valid");

    store
        .put_provenance_record(&record)
        .expect("record is stored");

    let retrieved = store
        .get_provenance_record("prov_01")
        .expect("get succeeds")
        .expect("record exists");

    assert_eq!(retrieved.provenance_id, record.provenance_id);
    assert_eq!(retrieved.operation, record.operation);
}

#[test]
fn store_roundtrip_event_log_entry_preserves_data() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let event = valid_event_log_entry("evt_01", "src_01", "prov_01");
    event.validate().expect("event is valid");

    store.put_event_log_entry(&event).expect("event is stored");

    let retrieved = store
        .get_event_log_entry("evt_01")
        .expect("get succeeds")
        .expect("event exists");

    assert_eq!(retrieved.event_id, event.event_id);
    assert_eq!(retrieved.target_ref, event.target_ref);
}

#[test]
fn lookup_by_id_returns_existing_source_ref() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source = valid_source_ref("src_01");
    store.put_source_ref(&source).expect("stored");

    let found = store
        .lookup_source_refs_by_id("src_01")
        .expect("lookup succeeds");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].source_id, "src_01");
}

#[test]
fn lookup_by_id_returns_none_for_missing_source_ref() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let found = store
        .lookup_source_refs_by_id("nonexistent")
        .expect("lookup succeeds");

    assert_eq!(found.len(), 0);
}

#[test]
fn lookup_by_content_hash_finds_source_ref() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source = valid_source_ref("src_01");
    store.put_source_ref(&source).expect("stored");

    let found = store
        .lookup_source_refs_by_content_hash(&hash())
        .expect("lookup succeeds");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].source_id, "src_01");
}

#[test]
fn lookup_by_origin_product_filters_correctly() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source1 = valid_source_ref("src_01");
    store.put_source_ref(&source1).expect("stored src_01");

    let mut source2 = valid_source_ref("src_02");
    source2.origin_product = "other-loader".to_string();
    store.put_source_ref(&source2).expect("stored src_02");

    let found = store
        .lookup_source_refs_by_origin_product("test-loader")
        .expect("lookup succeeds");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].source_id, "src_01");
}

#[test]
fn lookup_by_state_filters_correctly() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source1 = valid_source_ref("src_01");
    store.put_source_ref(&source1).expect("stored src_01");

    let mut source2 = valid_source_ref("src_02");
    source2.state = SourceState::Deleted;
    store.put_source_ref(&source2).expect("stored src_02");

    let active = store
        .lookup_source_refs_by_state(&SourceState::Active)
        .expect("lookup active succeeds");
    let deleted = store
        .lookup_source_refs_by_state(&SourceState::Deleted)
        .expect("lookup deleted succeeds");

    assert_eq!(active.len(), 1);
    assert_eq!(deleted.len(), 1);
}

#[test]
fn lookup_by_timestamp_range_filters_correctly() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let mut source1 = valid_source_ref("src_01");
    source1.created_at = "2026-06-30T00:00:00Z".to_string();
    store.put_source_ref(&source1).expect("stored");

    let mut source2 = valid_source_ref("src_02");
    source2.created_at = "2026-07-01T00:00:00Z".to_string();
    store.put_source_ref(&source2).expect("stored");

    let in_range = store
        .lookup_source_refs_by_timestamp_range("2026-06-29T00:00:00Z", "2026-06-30T12:00:00Z")
        .expect("lookup succeeds");

    assert_eq!(in_range.len(), 1);
    assert_eq!(in_range[0].source_id, "src_01");
}

#[test]
fn mark_deleted_transitions_state_and_creates_provenance() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source = valid_source_ref("src_01");
    store.put_source_ref(&source).expect("stored");

    let deletion_timestamp = "2026-07-01T00:00:00Z";
    store
        .mark_deleted("src_01", "GDPR deletion request", deletion_timestamp)
        .expect("mark deleted succeeds");

    let retrieved = store
        .get_source_ref("src_01")
        .expect("get succeeds")
        .expect("source still exists");

    assert_eq!(retrieved.state, SourceState::Deleted);

    // Provenance record should exist with Deleted operation
    let provenance_records = store
        .list_all_provenance_records()
        .expect("list provenances succeeds");

    let deletion_records: Vec<_> = provenance_records
        .iter()
        .filter(|r| r.operation == ProvenanceOperation::Deleted)
        .collect();

    assert!(!deletion_records.is_empty());
}

#[test]
fn mark_deleted_rejects_double_deletion() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source = valid_source_ref("src_01");
    store.put_source_ref(&source).expect("stored");

    let timestamp = "2026-07-01T00:00:00Z";
    store
        .mark_deleted("src_01", "first deletion", timestamp)
        .expect("first deletion succeeds");

    let error = store
        .mark_deleted("src_01", "second deletion attempt", timestamp)
        .expect_err("double deletion should fail");

    assert!(error.to_string().contains("already deleted"));
}

#[test]
fn mark_anonymized_transitions_state_and_creates_provenance() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source = valid_source_ref("src_01");
    store.put_source_ref(&source).expect("stored");

    let timestamp = "2026-07-01T00:00:00Z";
    store
        .mark_anonymized("src_01", "GDPR anonymization", timestamp)
        .expect("mark anonymized succeeds");

    let retrieved = store
        .get_source_ref("src_01")
        .expect("get succeeds")
        .expect("source still exists");

    assert_eq!(retrieved.state, SourceState::Anonymized);

    let provenance_records = store
        .list_all_provenance_records()
        .expect("list provenances succeeds");

    let anon_records: Vec<_> = provenance_records
        .iter()
        .filter(|r| r.operation == ProvenanceOperation::Anonymized)
        .collect();

    assert!(!anon_records.is_empty());
}

#[test]
fn lookup_excludes_deleted_from_active_searches() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = gear_memory::FileStore::new(temp_dir.path()).expect("create store");

    let source = valid_source_ref("src_01");
    store.put_source_ref(&source).expect("stored");

    store
        .mark_deleted("src_01", "removed", "2026-07-01T00:00:00Z")
        .expect("deleted");

    let active_results = store
        .lookup_source_refs_by_state(&SourceState::Active)
        .expect("lookup succeeds");

    assert_eq!(active_results.len(), 0);

    let deleted_results = store
        .lookup_source_refs_by_state(&SourceState::Deleted)
        .expect("lookup succeeds");

    assert_eq!(deleted_results.len(), 1);
}
