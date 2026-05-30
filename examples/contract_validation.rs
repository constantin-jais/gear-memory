use gear_memory::{SafeMetadata, SourceRef, SourceState, SourceType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = SourceRef {
        source_id: "src_01".to_string(),
        source_type: SourceType::Document,
        origin_product: "wrench-loader".to_string(),
        uri: Some("file:///tmp/source.md".to_string()),
        content_hash: format!("sha256:{}", "a".repeat(64)),
        provenance_id: "prov_01".to_string(),
        state: SourceState::Active,
        created_at: "2026-06-30T00:00:00Z".to_string(),
        metadata: SafeMetadata::default(),
    };

    source.validate()?;

    Ok(())
}
