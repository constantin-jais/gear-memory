use gear_memory::{
    EventLogEntry, ProvenanceOperation, ProvenanceRecord, SafeMetadata, SourceRef, SourceState,
    SourceType,
};

fn hash() -> String {
    format!("sha256:{}", "0".repeat(64))
}

#[test]
fn canvas_handoff_provenance_and_event_contracts_are_valid() {
    let source = SourceRef {
        source_id: "source:canvas-minimal.valid.json".to_string(),
        source_type: SourceType::Artifact,
        origin_product: "rumble-canvas".to_string(),
        uri: None,
        content_hash: hash(),
        provenance_id: "provenance:handoff-demo-valid".to_string(),
        state: SourceState::Active,
        created_at: "2026-06-30T00:00:00Z".to_string(),
        metadata: SafeMetadata::default(),
    };
    source.validate().expect("source ref is valid");

    let provenance = ProvenanceRecord {
        provenance_id: "provenance:handoff-demo-valid".to_string(),
        actor_ref: "actor-owner-demo".to_string(),
        operation: ProvenanceOperation::Exported,
        inputs: vec!["source:canvas-minimal.valid.json".to_string()],
        outputs: vec!["artifact:package-demo".to_string()],
        tool_ref: Some("rumble-canvas".to_string()),
        timestamp: "2026-06-30T00:00:00Z".to_string(),
        metadata: SafeMetadata::from_pairs([
            (
                "contract".to_string(),
                "implementation-handoff.v0.1".to_string(),
            ),
            (
                "fixture".to_string(),
                "canvas-minimal.valid.json".to_string(),
            ),
        ]),
    };
    provenance.validate().expect("provenance is valid");

    let event = EventLogEntry {
        event_id: "event:handoff-demo-valid:validated".to_string(),
        event_type: "implementation_handoff.validated".to_string(),
        actor_ref: "actor-owner-demo".to_string(),
        target_ref: "artifact:package-demo".to_string(),
        provenance_id: provenance.provenance_id.clone(),
        metadata: SafeMetadata::from_pairs([(
            "result".to_string(),
            "planning_only_valid".to_string(),
        )]),
        created_at: "2026-06-30T00:00:00Z".to_string(),
    };
    event.validate().expect("event is valid");

    assert!(provenance.stable_hash().starts_with("sha256:"));
}

#[test]
fn canvas_handoff_provenance_rejects_secret_like_metadata_keys() {
    let provenance = ProvenanceRecord {
        provenance_id: "provenance:handoff-demo-valid".to_string(),
        actor_ref: "actor-owner-demo".to_string(),
        operation: ProvenanceOperation::Exported,
        inputs: vec!["source:canvas-minimal.valid.json".to_string()],
        outputs: vec!["artifact:package-demo".to_string()],
        tool_ref: Some("rumble-canvas".to_string()),
        timestamp: "2026-06-30T00:00:00Z".to_string(),
        metadata: SafeMetadata::from_pairs([("api_token".to_string(), "not-stored".to_string())]),
    };

    assert!(provenance.validate().is_err());
}
