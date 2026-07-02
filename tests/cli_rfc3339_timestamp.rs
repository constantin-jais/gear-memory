/// Regression test: CLI erasure must produce valid RFC3339 timestamps.
///
/// The Stage 0 CLI computed "now" with hand-rolled calendar math whose
/// day-of-month formula overflows 31 (e.g. `2026-07-184T…`), which
/// `Store::mark_deleted` rejects — so every CLI erasure failed at runtime.
use std::process::Command;

use tempfile::TempDir;

use gear_memory::{
    FileStore, ProvenanceOperation, SafeMetadata, SourceRef, SourceState, SourceType, Store,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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

#[test]
fn cli_delete_writes_valid_rfc3339_timestamp() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store = FileStore::new(temp_dir.path()).expect("create store");
    store
        .put_source_ref(&valid_source_ref("src_01"))
        .expect("source is stored");

    let output = Command::new(env!("CARGO_BIN_EXE_gear-memory"))
        .args([
            "delete",
            temp_dir.path().to_str().expect("utf8 temp path"),
            "src_01",
            "GDPR erasure request",
        ])
        .output()
        .expect("run gear-memory delete");

    assert!(
        output.status.success(),
        "delete failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let source = store
        .get_source_ref("src_01")
        .expect("get succeeds")
        .expect("source still exists");
    assert_eq!(source.state, SourceState::Deleted);

    let records = store
        .list_all_provenance_records()
        .expect("list provenances succeeds");
    let deletions: Vec<_> = records
        .iter()
        .filter(|r| r.operation == ProvenanceOperation::Deleted)
        .collect();

    assert_eq!(deletions.len(), 1, "exactly one deletion provenance record");
    deletions[0]
        .validate()
        .expect("deletion provenance is a valid contract");
    OffsetDateTime::parse(&deletions[0].timestamp, &Rfc3339)
        .expect("deletion timestamp is valid RFC3339");
}
