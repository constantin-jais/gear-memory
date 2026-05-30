//! gear-memory — local-first source, memory, code graph, and provenance substrate.
//!
//! This crate deliberately stores and validates trustworthy references. It does
//! not decide what agents or products should do next.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Static project metadata used by the CLI and smoke tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectCard {
    pub name: &'static str,
    pub role: &'static str,
    pub responsibility: &'static str,
}

/// The repository's current scope card.
pub const PROJECT: ProjectCard = ProjectCard {
    name: "gear-memory",
    role: "local-first memory/source/code graph substrate",
    responsibility: "store, index, link, retrieve, and prove references; never decide",
};

/// Human-readable summary for CLI smoke runs.
pub fn summary() -> String {
    format!(
        "{} — {} ({})",
        PROJECT.name, PROJECT.role, PROJECT.responsibility
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    File,
    Url,
    FeedItem,
    NoteBlock,
    Transcript,
    Document,
    Dataset,
    Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    Active,
    Stale,
    Deleted,
    Anonymized,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub source_id: String,
    pub source_type: SourceType,
    pub origin_product: String,
    pub uri: Option<String>,
    pub content_hash: String,
    pub provenance_id: String,
    pub state: SourceState,
    pub created_at: String,
    #[serde(default)]
    pub metadata: SafeMetadata,
}

impl SourceRef {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("source_id", &self.source_id)?;
        validate_non_empty_field("origin_product", &self.origin_product)?;
        validate_non_empty_field("provenance_id", &self.provenance_id)?;
        validate_sha256_field("content_hash", &self.content_hash)?;
        validate_timestamp_field("created_at", &self.created_at)?;
        validate_metadata(&self.metadata)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceOperation {
    Created,
    Imported,
    Transformed,
    Indexed,
    Linked,
    StaleMarked,
    Exported,
    Signed,
    Distributed,
    Revoked,
    Deleted,
    Anonymized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    pub provenance_id: String,
    pub actor_ref: String,
    pub operation: ProvenanceOperation,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub tool_ref: Option<String>,
    pub timestamp: String,
    pub metadata: SafeMetadata,
}

impl ProvenanceRecord {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("provenance_id", &self.provenance_id)?;
        validate_non_empty_field("actor_ref", &self.actor_ref)?;
        validate_non_empty_list("outputs", &self.outputs)?;
        validate_timestamp_field("timestamp", &self.timestamp)?;
        validate_metadata(&self.metadata)
    }

    pub fn stable_hash(&self) -> String {
        stable_json_hash(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    Pending,
    Indexed,
    Stale,
    Deleted,
    Anonymized,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub schema_version: String,
    pub chunk_count: u32,
    pub embedding_model_ref: Option<String>,
    pub indexed_at: Option<String>,
}

impl IndexMetadata {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        if self.schema_version != "memory-entry.v0.1" {
            return Err(ContractValidationError::InvalidSchemaVersion {
                field: "index_metadata.schema_version",
                value: self.schema_version.clone(),
            });
        }

        if let Some(indexed_at) = &self.indexed_at {
            validate_timestamp_field("index_metadata.indexed_at", indexed_at)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub memory_entry_id: String,
    pub source_ref: String,
    pub content_hash: String,
    pub index_state: IndexState,
    pub index_metadata: IndexMetadata,
    pub created_at: String,
}

impl MemoryEntry {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("memory_entry_id", &self.memory_entry_id)?;
        validate_non_empty_field("source_ref", &self.source_ref)?;
        validate_sha256_field("content_hash", &self.content_hash)?;
        self.index_metadata.validate()?;
        validate_timestamp_field("created_at", &self.created_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub event_id: String,
    pub event_type: String,
    pub actor_ref: String,
    pub target_ref: String,
    pub provenance_id: String,
    pub metadata: SafeMetadata,
    pub created_at: String,
}

impl EventLogEntry {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("event_id", &self.event_id)?;
        validate_non_empty_field("event_type", &self.event_type)?;
        validate_non_empty_field("actor_ref", &self.actor_ref)?;
        validate_non_empty_field("target_ref", &self.target_ref)?;
        validate_non_empty_field("provenance_id", &self.provenance_id)?;
        validate_timestamp_field("created_at", &self.created_at)?;
        validate_metadata(&self.metadata)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeMap {
    pub code_map_id: String,
    pub root_source_ref: String,
    pub scope: CodeMapScope,
    pub parser_refs: Vec<String>,
    pub symbols: Vec<CodeSymbol>,
    pub edges: Vec<CodeEdge>,
    pub state: CodeMapState,
    pub created_at: String,
}

impl CodeMap {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("code_map_id", &self.code_map_id)?;
        validate_non_empty_field("root_source_ref", &self.root_source_ref)?;
        self.scope.validate()?;
        validate_non_empty_list("parser_refs", &self.parser_refs)?;
        for parser_ref in &self.parser_refs {
            validate_non_empty_field("parser_refs[]", parser_ref)?;
        }
        for symbol in &self.symbols {
            symbol.validate()?;
        }
        for edge in &self.edges {
            edge.validate()?;
        }
        validate_timestamp_field("created_at", &self.created_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeMapScope {
    pub repo_ref: Option<String>,
    pub revision: String,
    pub paths: Vec<String>,
}

impl CodeMapScope {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("scope.revision", &self.revision)?;
        validate_non_empty_list("scope.paths", &self.paths)?;
        for path in &self.paths {
            validate_non_empty_field("scope.paths[]", path)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeMapState {
    Active,
    Stale,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub symbol_id: String,
    pub kind: CodeSymbolKind,
    pub name: String,
    pub source_ref: String,
    pub range: SourceRange,
    pub content_hash: String,
}

impl CodeSymbol {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("symbol_id", &self.symbol_id)?;
        validate_non_empty_field("name", &self.name)?;
        validate_non_empty_field("source_ref", &self.source_ref)?;
        self.range.validate()?;
        validate_sha256_field("content_hash", &self.content_hash)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeSymbolKind {
    Function,
    Type,
    Module,
    Trait,
    Interface,
    Route,
    Table,
    Test,
    Config,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start_line: u32,
    pub end_line: u32,
}

impl SourceRange {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        if self.start_line == 0 {
            return Err(ContractValidationError::InvalidRange("start_line"));
        }
        if self.end_line == 0 || self.end_line < self.start_line {
            return Err(ContractValidationError::InvalidRange("end_line"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeEdge {
    pub from: String,
    pub to: String,
    pub kind: CodeEdgeKind,
}

impl CodeEdge {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        validate_non_empty_field("edge.from", &self.from)?;
        validate_non_empty_field("edge.to", &self.to)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeEdgeKind {
    Defines,
    Calls,
    Imports,
    Tests,
    Configures,
    Documents,
    GeneratedFrom,
    BelongsTo,
    Cites,
    DerivedFrom,
    Supersedes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GearMemoryBundle {
    pub format: String,
    #[serde(default)]
    pub source_refs: Vec<SourceRef>,
    #[serde(default)]
    pub memory_entries: Vec<MemoryEntry>,
    #[serde(default)]
    pub event_log_entries: Vec<EventLogEntry>,
    #[serde(default)]
    pub code_maps: Vec<CodeMap>,
    #[serde(default)]
    pub provenance_records: Vec<ProvenanceRecord>,
}

impl GearMemoryBundle {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        if self.format != "gear.memory.v0.1" {
            return Err(ContractValidationError::InvalidSchemaVersion {
                field: "format",
                value: self.format.clone(),
            });
        }

        for source in &self.source_refs {
            source.validate()?;
        }
        for entry in &self.memory_entries {
            entry.validate()?;
        }
        for event in &self.event_log_entries {
            event.validate()?;
        }
        for code_map in &self.code_maps {
            code_map.validate()?;
        }
        for provenance in &self.provenance_records {
            provenance.validate()?;
        }

        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SafeMetadata {
    #[serde(flatten)]
    values: BTreeMap<String, Value>,
}

impl SafeMetadata {
    pub fn from_pairs<const N: usize>(pairs: [(String, String); N]) -> Self {
        Self {
            values: pairs
                .into_iter()
                .map(|(key, value)| (key, Value::String(value)))
                .collect(),
        }
    }

    pub fn from_values(values: BTreeMap<String, Value>) -> Self {
        Self { values }
    }

    pub fn validate(&self) -> Result<(), MetadataValidationError> {
        for key in self.values.keys() {
            if is_secret_like_key(key) {
                return Err(MetadataValidationError::SecretLikeKey(key.clone()));
            }
        }

        Ok(())
    }

    pub fn stable_hash(&self) -> String {
        stable_json_hash(&self.values)
    }
}

impl fmt::Debug for SafeMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted = self
            .values
            .keys()
            .map(|key| (key, "<redacted>"))
            .collect::<BTreeMap<_, _>>();

        formatter
            .debug_tuple("SafeMetadata")
            .field(&redacted)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataValidationError {
    SecretLikeKey(String),
}

impl fmt::Display for MetadataValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SecretLikeKey(key) => {
                write!(formatter, "metadata key `{key}` may contain a secret")
            }
        }
    }
}

impl std::error::Error for MetadataValidationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractValidationError {
    EmptyField(&'static str),
    EmptyList(&'static str),
    SecretLikeMetadataKey(String),
    MalformedSha256 { field: &'static str, value: String },
    MalformedTimestamp { field: &'static str, value: String },
    InvalidSchemaVersion { field: &'static str, value: String },
    InvalidRange(&'static str),
}

impl fmt::Display for ContractValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "field `{field}` must not be empty"),
            Self::EmptyList(field) => write!(formatter, "list `{field}` must not be empty"),
            Self::SecretLikeMetadataKey(key) => {
                write!(formatter, "metadata key `{key}` may contain a secret")
            }
            Self::MalformedSha256 { field, value } => {
                write!(formatter, "field `{field}` is not a sha256 hash: `{value}`")
            }
            Self::MalformedTimestamp { field, value } => {
                write!(formatter, "field `{field}` is not RFC3339: `{value}`")
            }
            Self::InvalidSchemaVersion { field, value } => {
                write!(
                    formatter,
                    "field `{field}` has unsupported schema/version: `{value}`"
                )
            }
            Self::InvalidRange(field) => write!(formatter, "range field `{field}` is invalid"),
        }
    }
}

impl std::error::Error for ContractValidationError {}

fn is_secret_like_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    let normalized = normalized.replace('-', "_");

    normalized == "secret"
        || normalized == "token"
        || normalized == "password"
        || normalized == "credential"
        || normalized == "api_key"
        || normalized == "raw_log"
        || normalized.ends_with("_secret")
        || normalized.ends_with("_token")
        || normalized.ends_with("_password")
        || normalized.ends_with("_credential")
        || normalized.ends_with("_api_key")
        || normalized.ends_with("_raw_log")
        || normalized.contains("secret_value")
        || normalized.contains("token_value")
        || normalized.contains("password_value")
        || normalized.contains("credential_value")
        || normalized.contains("api_key_value")
        || normalized.contains("raw_log_value")
}

fn validate_sha256_field(field: &'static str, value: &str) -> Result<(), ContractValidationError> {
    if is_valid_sha256(value) {
        return Ok(());
    }

    Err(ContractValidationError::MalformedSha256 {
        field,
        value: value.to_string(),
    })
}

fn validate_timestamp_field(
    field: &'static str,
    value: &str,
) -> Result<(), ContractValidationError> {
    if OffsetDateTime::parse(value, &Rfc3339).is_ok() {
        return Ok(());
    }

    Err(ContractValidationError::MalformedTimestamp {
        field,
        value: value.to_string(),
    })
}

fn validate_non_empty_field(
    field: &'static str,
    value: &str,
) -> Result<(), ContractValidationError> {
    if value.trim().is_empty() {
        return Err(ContractValidationError::EmptyField(field));
    }

    Ok(())
}

fn validate_non_empty_list<T>(
    field: &'static str,
    values: &[T],
) -> Result<(), ContractValidationError> {
    if values.is_empty() {
        return Err(ContractValidationError::EmptyList(field));
    }

    Ok(())
}

fn validate_metadata(metadata: &SafeMetadata) -> Result<(), ContractValidationError> {
    metadata.validate().map_err(|error| match error {
        MetadataValidationError::SecretLikeKey(key) => {
            ContractValidationError::SecretLikeMetadataKey(key)
        }
    })
}

fn is_valid_sha256(value: &str) -> bool {
    const PREFIX: &str = "sha256:";
    let Some(hex) = value.strip_prefix(PREFIX) else {
        return false;
    };

    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn stable_json_hash<T>(value: &T) -> String
where
    T: Serialize,
{
    let canonical_json = serde_json::to_string(value).expect("serializable contract value");
    let digest = Sha256::digest(canonical_json.as_bytes());

    format!("sha256:{}", to_lower_hex(&digest))
}

fn to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> String {
        format!("sha256:{}", "a".repeat(64))
    }

    #[test]
    fn project_card_names_the_repo_and_responsibility() {
        assert_eq!(PROJECT.name, "gear-memory");
        assert!(summary().contains(PROJECT.role));
        assert!(summary().contains("never decide"));
    }

    #[test]
    fn source_ref_roundtrips_with_revoked_state() {
        let mut source = valid_source_ref();
        source.state = SourceState::Revoked;

        let encoded = serde_json::to_string(&source).expect("source serializes");
        let decoded: SourceRef = serde_json::from_str(&encoded).expect("source deserializes");

        assert_eq!(decoded, source);
    }

    #[test]
    fn memory_entry_rejects_missing_required_source_ref() {
        let payload = r#"{
            "memory_entry_id": "mem_01",
            "content_hash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "index_state": "indexed",
            "index_metadata": {
                "schema_version": "memory-entry.v0.1",
                "chunk_count": 2,
                "embedding_model_ref": null,
                "indexed_at": "2026-06-30T00:01:00Z"
            },
            "created_at": "2026-06-30T00:00:00Z"
        }"#;

        let error =
            serde_json::from_str::<MemoryEntry>(payload).expect_err("source_ref is required");

        assert!(error.to_string().contains("missing field `source_ref`"));
    }

    #[test]
    fn event_log_debug_redacts_metadata_values() {
        let event = EventLogEntry {
            event_id: "evt_01".to_string(),
            event_type: "memory.indexed".to_string(),
            actor_ref: "actor_01".to_string(),
            target_ref: "mem_01".to_string(),
            provenance_id: "prov_01".to_string(),
            metadata: SafeMetadata::from_pairs([(
                "provider_metadata_without_secrets".to_string(),
                "token-should-not-leak".to_string(),
            )]),
            created_at: "2026-06-30T00:00:00Z".to_string(),
        };

        let debug = format!("{event:?}");

        assert!(debug.contains("provider_metadata_without_secrets"));
        assert!(!debug.contains("token-should-not-leak"));
    }

    #[test]
    fn metadata_validation_rejects_secret_like_keys() {
        let metadata = SafeMetadata::from_pairs([(
            "api_token".to_string(),
            "token-should-not-be-stored".to_string(),
        )]);

        let error = metadata
            .validate()
            .expect_err("secret-like metadata keys are rejected");

        assert_eq!(
            error,
            MetadataValidationError::SecretLikeKey("api_token".to_string())
        );
    }

    #[test]
    fn metadata_allows_safe_count_keys() {
        let metadata = SafeMetadata::from_pairs([("secret_count".to_string(), "0".to_string())]);

        metadata
            .validate()
            .expect("safe summary counts are allowed");
    }

    #[test]
    fn provenance_record_stable_hash_ignores_metadata_insertion_order() {
        let mut left = indexed_provenance_record();
        left.metadata = SafeMetadata::from_pairs([
            ("runner".to_string(), "local".to_string()),
            ("tool".to_string(), "wrench-loader".to_string()),
        ]);

        let mut right = indexed_provenance_record();
        right.metadata = SafeMetadata::from_pairs([
            ("tool".to_string(), "wrench-loader".to_string()),
            ("runner".to_string(), "local".to_string()),
        ]);

        assert_eq!(left.stable_hash(), right.stable_hash());
    }

    #[test]
    fn source_ref_validation_rejects_malformed_content_hash() {
        let mut source = valid_source_ref();
        source.content_hash = "sha256:not-hex".to_string();

        let error = source
            .validate()
            .expect_err("malformed content hash is rejected");

        assert_eq!(
            error,
            ContractValidationError::MalformedSha256 {
                field: "content_hash",
                value: "sha256:not-hex".to_string()
            }
        );
    }

    #[test]
    fn memory_entry_validation_rejects_wrong_schema_version() {
        let mut entry = valid_memory_entry();
        entry.index_metadata.schema_version = "memory-entry.v9".to_string();

        let error = entry
            .validate()
            .expect_err("wrong schema version is rejected");

        assert_eq!(
            error,
            ContractValidationError::InvalidSchemaVersion {
                field: "index_metadata.schema_version",
                value: "memory-entry.v9".to_string()
            }
        );
    }

    #[test]
    fn code_map_validation_rejects_invalid_symbol_range() {
        let mut code_map = valid_code_map();
        code_map.symbols[0].range.end_line = 0;

        let error = code_map
            .validate()
            .expect_err("invalid symbol range is rejected");

        assert_eq!(error, ContractValidationError::InvalidRange("end_line"));
    }

    #[test]
    fn bundle_validation_accepts_p0_contract_family() {
        let bundle = GearMemoryBundle {
            format: "gear.memory.v0.1".to_string(),
            source_refs: vec![valid_source_ref()],
            memory_entries: vec![valid_memory_entry()],
            event_log_entries: vec![valid_event_log_entry()],
            code_maps: vec![valid_code_map()],
            provenance_records: vec![indexed_provenance_record()],
        };

        bundle.validate().expect("valid bundle is accepted");
    }

    #[test]
    fn bundle_validation_rejects_unknown_format() {
        let bundle = GearMemoryBundle {
            format: "gear.memory.v9".to_string(),
            source_refs: vec![],
            memory_entries: vec![],
            event_log_entries: vec![],
            code_maps: vec![],
            provenance_records: vec![],
        };

        let error = bundle.validate().expect_err("unknown format is rejected");

        assert_eq!(
            error,
            ContractValidationError::InvalidSchemaVersion {
                field: "format",
                value: "gear.memory.v9".to_string()
            }
        );
    }

    fn valid_source_ref() -> SourceRef {
        SourceRef {
            source_id: "src_01".to_string(),
            source_type: SourceType::Document,
            origin_product: "wrench-loader".to_string(),
            uri: Some("file:///tmp/source.md".to_string()),
            content_hash: hash(),
            provenance_id: "prov_01".to_string(),
            state: SourceState::Active,
            created_at: "2026-06-30T00:00:00Z".to_string(),
            metadata: SafeMetadata::default(),
        }
    }

    fn valid_memory_entry() -> MemoryEntry {
        MemoryEntry {
            memory_entry_id: "mem_01".to_string(),
            source_ref: "src_01".to_string(),
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

    fn indexed_provenance_record() -> ProvenanceRecord {
        ProvenanceRecord {
            provenance_id: "prov_01".to_string(),
            actor_ref: "actor_01".to_string(),
            operation: ProvenanceOperation::Indexed,
            inputs: vec!["src_01".to_string()],
            outputs: vec!["mem_01".to_string()],
            tool_ref: Some("gear-memory".to_string()),
            timestamp: "2026-06-30T00:00:00Z".to_string(),
            metadata: SafeMetadata::from_pairs([("runner".to_string(), "local".to_string())]),
        }
    }

    fn valid_event_log_entry() -> EventLogEntry {
        EventLogEntry {
            event_id: "evt_01".to_string(),
            event_type: "memory.indexed".to_string(),
            actor_ref: "actor_01".to_string(),
            target_ref: "mem_01".to_string(),
            provenance_id: "prov_01".to_string(),
            metadata: SafeMetadata::from_pairs([("result".to_string(), "ok".to_string())]),
            created_at: "2026-06-30T00:00:00Z".to_string(),
        }
    }

    fn valid_code_map() -> CodeMap {
        CodeMap {
            code_map_id: "cm_01".to_string(),
            root_source_ref: "src_01".to_string(),
            scope: CodeMapScope {
                repo_ref: Some("repo_demo".to_string()),
                revision: "git:abc123".to_string(),
                paths: vec!["src/".to_string()],
            },
            parser_refs: vec!["tree-sitter:rust@0.0.0-demo".to_string()],
            symbols: vec![CodeSymbol {
                symbol_id: "sym_01".to_string(),
                kind: CodeSymbolKind::Function,
                name: "demo::main".to_string(),
                source_ref: "src_01".to_string(),
                range: SourceRange {
                    start_line: 1,
                    end_line: 3,
                },
                content_hash: hash(),
            }],
            edges: vec![],
            state: CodeMapState::Active,
            created_at: "2026-06-30T00:00:00Z".to_string(),
        }
    }
}
