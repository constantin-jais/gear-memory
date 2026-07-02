//! SQLite-backed `Store`: the "graph rung" of the progressive-indexing
//! ladder. Same persistence contract as `FileStore`, plus indexed
//! code-graph queries that a per-entity JSON file layout cannot answer
//! efficiently.
//!
//! Engine choice (rusqlite `bundled` vs stoolap vs redb, measured), schema
//! shape, and query surface are recorded in
//! `docs/adr/0002-sqlite-code-index.md`.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::{
    CodeEdge, CodeEdgeKind, CodeMap, CodeSymbol, CodeSymbolKind, ContractValidationError,
    EventLogEntry, GearMemoryBundle, IndexState, MemoryEntry, ProvenanceRecord, SourceRange,
    SourceRef, SourceState, Store, StoreError,
};

/// serde tokens of every `CodeSymbolKind` variant — update alongside the
/// contract enum (the unit test below catches renames and removals).
const SYMBOL_KIND_TOKENS: [&str; 10] = [
    "function",
    "type",
    "module",
    "trait",
    "interface",
    "route",
    "table",
    "test",
    "config",
    "file",
];

/// serde tokens of every `CodeEdgeKind` variant — same maintenance rule.
const EDGE_KIND_TOKENS: [&str; 11] = [
    "defines",
    "calls",
    "imports",
    "tests",
    "configures",
    "documents",
    "generated_from",
    "belongs_to",
    "cites",
    "derived_from",
    "supersedes",
];

const ENTITY_TABLES: [&str; 7] = [
    "source_refs",
    "memory_entries",
    "provenance_records",
    "event_log_entries",
    "code_maps",
    "code_symbols",
    "code_edges",
];

const SCHEMA_VERSION: i64 = 1;

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS source_refs (
    source_id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    origin_product TEXT NOT NULL,
    state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    record_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_source_refs_content_hash ON source_refs (content_hash);
CREATE INDEX IF NOT EXISTS idx_source_refs_origin_product ON source_refs (origin_product);
CREATE INDEX IF NOT EXISTS idx_source_refs_state ON source_refs (state);
CREATE TABLE IF NOT EXISTS memory_entries (
    memory_entry_id TEXT PRIMARY KEY,
    index_state TEXT NOT NULL,
    record_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_memory_entries_state ON memory_entries (index_state);
CREATE TABLE IF NOT EXISTS provenance_records (
    provenance_id TEXT PRIMARY KEY,
    record_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS event_log_entries (
    event_id TEXT PRIMARY KEY,
    record_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS code_maps (
    code_map_id TEXT PRIMARY KEY,
    state TEXT NOT NULL,
    record_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS code_symbols (
    code_map_id TEXT NOT NULL,
    symbol_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    PRIMARY KEY (code_map_id, symbol_id)
);
CREATE INDEX IF NOT EXISTS idx_code_symbols_name ON code_symbols (name);
CREATE INDEX IF NOT EXISTS idx_code_symbols_kind ON code_symbols (kind);
CREATE TABLE IF NOT EXISTS code_edges (
    code_map_id TEXT NOT NULL,
    from_symbol TEXT NOT NULL,
    to_symbol TEXT NOT NULL,
    kind TEXT NOT NULL,
    PRIMARY KEY (code_map_id, from_symbol, to_symbol, kind)
);
CREATE INDEX IF NOT EXISTS idx_code_edges_from ON code_edges (code_map_id, from_symbol);
CREATE INDEX IF NOT EXISTS idx_code_edges_to ON code_edges (code_map_id, to_symbol);
";

/// SQLite-backed store. One database file, WAL journaling, contracts
/// validated before every write, canonical JSON kept per record for
/// lossless roundtrips while indexed columns serve the queries.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

/// Direction of edge traversal for neighbor queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeDirection {
    In,
    Out,
    Both,
}

/// One BFS hop: how deep a symbol was first reached from the start symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceHop {
    pub depth: u32,
    pub symbol_id: String,
}

/// Deterministic store statistics; zero counts are kept on purpose —
/// a zero is a finding, not an omission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreStats {
    pub schema_version: i64,
    pub entities: std::collections::BTreeMap<String, u64>,
    pub symbols_by_kind: std::collections::BTreeMap<String, u64>,
    pub edges_by_kind: std::collections::BTreeMap<String, u64>,
}

/// Counts of records written by a bundle ingestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngestReport {
    pub source_refs: u64,
    pub memory_entries: u64,
    pub provenance_records: u64,
    pub event_log_entries: u64,
    pub code_maps: u64,
    pub code_symbols: u64,
    pub code_edges: u64,
}

fn sql_err(error: rusqlite::Error) -> StoreError {
    StoreError::IoError(error.to_string())
}

fn ser_err(error: serde_json::Error) -> StoreError {
    StoreError::SerializationError(error.to_string())
}

fn de_err(error: serde_json::Error) -> StoreError {
    StoreError::DeserializationError(error.to_string())
}

fn validation_err(error: ContractValidationError) -> StoreError {
    StoreError::InvalidOperation(error.to_string())
}

/// serde string token ("active", "function", …) of a string-serializing enum.
fn enum_token<T: Serialize>(value: &T) -> Result<String, StoreError> {
    match serde_json::to_value(value).map_err(ser_err)? {
        serde_json::Value::String(token) => Ok(token),
        other => Err(StoreError::SerializationError(format!(
            "expected a string token, got {other}"
        ))),
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(ser_err)
}

fn symbol_kind_from_token(token: &str) -> Result<CodeSymbolKind, StoreError> {
    serde_json::from_value(serde_json::Value::String(token.to_string())).map_err(de_err)
}

fn edge_kind_from_token(token: &str) -> Result<CodeEdgeKind, StoreError> {
    serde_json::from_value(serde_json::Value::String(token.to_string())).map_err(de_err)
}

fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, StoreError> {
    serde_json::from_str(json).map_err(de_err)
}

fn insert_source_ref(conn: &Connection, source: &SourceRef) -> Result<(), StoreError> {
    source.validate().map_err(validation_err)?;

    conn.execute(
        "INSERT OR REPLACE INTO source_refs
         (source_id, content_hash, origin_product, state, created_at, record_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source.source_id,
            source.content_hash,
            source.origin_product,
            enum_token(&source.state)?,
            source.created_at,
            to_json(source)?,
        ],
    )
    .map_err(sql_err)?;

    Ok(())
}

fn insert_memory_entry(conn: &Connection, entry: &MemoryEntry) -> Result<(), StoreError> {
    entry.validate().map_err(validation_err)?;

    conn.execute(
        "INSERT OR REPLACE INTO memory_entries (memory_entry_id, index_state, record_json)
         VALUES (?1, ?2, ?3)",
        params![
            entry.memory_entry_id,
            enum_token(&entry.index_state)?,
            to_json(entry)?,
        ],
    )
    .map_err(sql_err)?;

    Ok(())
}

fn insert_provenance_record(
    conn: &Connection,
    record: &ProvenanceRecord,
) -> Result<(), StoreError> {
    record.validate().map_err(validation_err)?;

    conn.execute(
        "INSERT OR REPLACE INTO provenance_records (provenance_id, record_json) VALUES (?1, ?2)",
        params![record.provenance_id, to_json(record)?],
    )
    .map_err(sql_err)?;

    Ok(())
}

fn insert_event_log_entry(conn: &Connection, event: &EventLogEntry) -> Result<(), StoreError> {
    event.validate().map_err(validation_err)?;

    conn.execute(
        "INSERT OR REPLACE INTO event_log_entries (event_id, record_json) VALUES (?1, ?2)",
        params![event.event_id, to_json(event)?],
    )
    .map_err(sql_err)?;

    Ok(())
}

/// Replace semantics: the code map row plus all its normalized symbols and
/// edges are rewritten atomically (callers wrap this in a transaction).
fn insert_code_map(conn: &Connection, code_map: &CodeMap) -> Result<(), StoreError> {
    code_map.validate().map_err(validation_err)?;

    conn.execute(
        "INSERT OR REPLACE INTO code_maps (code_map_id, state, record_json) VALUES (?1, ?2, ?3)",
        params![
            code_map.code_map_id,
            enum_token(&code_map.state)?,
            to_json(code_map)?,
        ],
    )
    .map_err(sql_err)?;

    conn.execute(
        "DELETE FROM code_symbols WHERE code_map_id = ?1",
        params![code_map.code_map_id],
    )
    .map_err(sql_err)?;
    conn.execute(
        "DELETE FROM code_edges WHERE code_map_id = ?1",
        params![code_map.code_map_id],
    )
    .map_err(sql_err)?;

    for symbol in &code_map.symbols {
        conn.execute(
            "INSERT OR REPLACE INTO code_symbols
             (code_map_id, symbol_id, kind, name, source_ref, start_line, end_line, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                code_map.code_map_id,
                symbol.symbol_id,
                enum_token(&symbol.kind)?,
                symbol.name,
                symbol.source_ref,
                symbol.range.start_line,
                symbol.range.end_line,
                symbol.content_hash,
            ],
        )
        .map_err(sql_err)?;
    }

    for edge in &code_map.edges {
        conn.execute(
            "INSERT OR REPLACE INTO code_edges (code_map_id, from_symbol, to_symbol, kind)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                code_map.code_map_id,
                edge.from,
                edge.to,
                enum_token(&edge.kind)?,
            ],
        )
        .map_err(sql_err)?;
    }

    Ok(())
}

fn get_record_json<T: DeserializeOwned>(
    conn: &Connection,
    sql: &str,
    id: &str,
) -> Result<Option<T>, StoreError> {
    let json: Option<String> = conn
        .query_row(sql, params![id], |row| row.get(0))
        .optional()
        .map_err(sql_err)?;

    json.map(|payload| from_json(&payload)).transpose()
}

fn collect_records<T: DeserializeOwned>(
    conn: &Connection,
    sql: &str,
    query_params: &[&dyn rusqlite::ToSql],
) -> Result<Vec<T>, StoreError> {
    let mut stmt = conn.prepare_cached(sql).map_err(sql_err)?;
    let rows = stmt
        .query_map(query_params, |row| row.get::<_, String>(0))
        .map_err(sql_err)?;

    let mut records = Vec::new();
    for row in rows {
        let json = row.map_err(sql_err)?;
        records.push(from_json(&json)?);
    }

    Ok(records)
}

impl SqliteStore {
    /// Open (or create) the database at `db_path`, creating parent
    /// directories as needed. WAL journaling; schema tracked through
    /// `PRAGMA user_version`.
    pub fn new(db_path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = db_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::IoError(e.to_string()))?;
        }

        let conn = Connection::open(db_path).map_err(sql_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(sql_err)?;

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(sql_err)?;
        match version {
            0 => {
                conn.execute_batch(SCHEMA_SQL).map_err(sql_err)?;
                conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                    .map_err(sql_err)?;
            }
            SCHEMA_VERSION => {}
            other => {
                return Err(StoreError::InvalidOperation(format!(
                    "unsupported schema version {other} (supported: {SCHEMA_VERSION})"
                )));
            }
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn
            .lock()
            .map_err(|_| StoreError::IoError("sqlite connection mutex poisoned".to_string()))
    }

    /// Ingest a validated bundle atomically, recording the ingestion
    /// provenance in the same transaction.
    pub fn ingest_bundle(
        &self,
        bundle: &GearMemoryBundle,
        ingest_provenance: &ProvenanceRecord,
    ) -> Result<IngestReport, StoreError> {
        bundle.validate().map_err(validation_err)?;
        ingest_provenance.validate().map_err(validation_err)?;

        let mut guard = self.lock()?;
        let tx = guard.transaction().map_err(sql_err)?;

        for source in &bundle.source_refs {
            insert_source_ref(&tx, source)?;
        }
        for entry in &bundle.memory_entries {
            insert_memory_entry(&tx, entry)?;
        }
        for event in &bundle.event_log_entries {
            insert_event_log_entry(&tx, event)?;
        }
        for record in &bundle.provenance_records {
            insert_provenance_record(&tx, record)?;
        }
        let mut code_symbols = 0u64;
        let mut code_edges = 0u64;
        for code_map in &bundle.code_maps {
            insert_code_map(&tx, code_map)?;
            code_symbols += code_map.symbols.len() as u64;
            code_edges += code_map.edges.len() as u64;
        }
        insert_provenance_record(&tx, ingest_provenance)?;
        tx.commit().map_err(sql_err)?;

        Ok(IngestReport {
            source_refs: bundle.source_refs.len() as u64,
            memory_entries: bundle.memory_entries.len() as u64,
            provenance_records: bundle.provenance_records.len() as u64 + 1,
            event_log_entries: bundle.event_log_entries.len() as u64,
            code_maps: bundle.code_maps.len() as u64,
            code_symbols,
            code_edges,
        })
    }

    /// Symbols whose name contains `name_contains` (plain substring — no
    /// pattern metacharacters), optionally filtered by kind; deterministic
    /// order (name, code_map_id, symbol_id). Returns (code_map_id, symbol).
    pub fn symbol_search(
        &self,
        name_contains: &str,
        kind: Option<&CodeSymbolKind>,
    ) -> Result<Vec<(String, CodeSymbol)>, StoreError> {
        let kind_token = kind.map(enum_token).transpose()?;
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare_cached(
                "SELECT code_map_id, symbol_id, kind, name, source_ref, start_line, end_line, content_hash
                 FROM code_symbols
                 WHERE instr(name, ?1) > 0 AND (?2 IS NULL OR kind = ?2)
                 ORDER BY name, code_map_id, symbol_id",
            )
            .map_err(sql_err)?;

        let rows = stmt
            .query_map(params![name_contains, kind_token], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, u32>(5)?,
                    row.get::<_, u32>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(sql_err)?;

        let mut hits = Vec::new();
        for row in rows {
            let (
                code_map_id,
                symbol_id,
                token,
                name,
                source_ref,
                start_line,
                end_line,
                content_hash,
            ) = row.map_err(sql_err)?;
            hits.push((
                code_map_id,
                CodeSymbol {
                    symbol_id,
                    kind: symbol_kind_from_token(&token)?,
                    name,
                    source_ref,
                    range: SourceRange {
                        start_line,
                        end_line,
                    },
                    content_hash,
                },
            ));
        }

        Ok(hits)
    }

    /// Edges touching `symbol_id` in the given direction, optionally
    /// filtered by kind; deterministic order (from, to, kind).
    pub fn symbol_neighbors(
        &self,
        code_map_id: &str,
        symbol_id: &str,
        direction: EdgeDirection,
        kind: Option<&CodeEdgeKind>,
    ) -> Result<Vec<CodeEdge>, StoreError> {
        let kind_token = kind.map(enum_token).transpose()?;
        let sql = match direction {
            EdgeDirection::Out => {
                "SELECT from_symbol, to_symbol, kind FROM code_edges
                 WHERE code_map_id = ?1 AND from_symbol = ?2 AND (?3 IS NULL OR kind = ?3)
                 ORDER BY from_symbol, to_symbol, kind"
            }
            EdgeDirection::In => {
                "SELECT from_symbol, to_symbol, kind FROM code_edges
                 WHERE code_map_id = ?1 AND to_symbol = ?2 AND (?3 IS NULL OR kind = ?3)
                 ORDER BY from_symbol, to_symbol, kind"
            }
            EdgeDirection::Both => {
                "SELECT from_symbol, to_symbol, kind FROM code_edges
                 WHERE code_map_id = ?1 AND (from_symbol = ?2 OR to_symbol = ?2)
                   AND (?3 IS NULL OR kind = ?3)
                 ORDER BY from_symbol, to_symbol, kind"
            }
        };

        let conn = self.lock()?;
        let mut stmt = conn.prepare_cached(sql).map_err(sql_err)?;
        let rows = stmt
            .query_map(params![code_map_id, symbol_id, kind_token], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(sql_err)?;

        let mut edges = Vec::new();
        for row in rows {
            let (from, to, token) = row.map_err(sql_err)?;
            edges.push(CodeEdge {
                from,
                to,
                kind: edge_kind_from_token(&token)?,
            });
        }

        Ok(edges)
    }

    /// Breadth-first traversal over outgoing edges, bounded by `max_depth`.
    /// Deterministic: within a depth level, symbols are visited in
    /// lexicographic order. The start symbol is hop 0.
    pub fn trace_bfs(
        &self,
        code_map_id: &str,
        start_symbol_id: &str,
        max_depth: u32,
    ) -> Result<Vec<TraceHop>, StoreError> {
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(start_symbol_id.to_string());

        let mut hops = vec![TraceHop {
            depth: 0,
            symbol_id: start_symbol_id.to_string(),
        }];
        let mut frontier = vec![start_symbol_id.to_string()];

        for depth in 1..=max_depth {
            let mut next: BTreeSet<String> = BTreeSet::new();
            for node in &frontier {
                for edge in self.symbol_neighbors(code_map_id, node, EdgeDirection::Out, None)? {
                    if !visited.contains(&edge.to) {
                        next.insert(edge.to);
                    }
                }
            }

            if next.is_empty() {
                break;
            }

            frontier = Vec::with_capacity(next.len());
            for symbol_id in next {
                visited.insert(symbol_id.clone());
                hops.push(TraceHop {
                    depth,
                    symbol_id: symbol_id.clone(),
                });
                frontier.push(symbol_id);
            }
        }

        Ok(hops)
    }

    /// Deterministic counts per entity table and per symbol/edge kind.
    /// Zero counts are kept: a zero is a finding, not an omission.
    pub fn stats(&self) -> Result<StoreStats, StoreError> {
        let conn = self.lock()?;

        let mut entities = BTreeMap::new();
        for table in ENTITY_TABLES {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .map_err(sql_err)?;
            entities.insert(table.to_string(), count as u64);
        }

        let mut symbols_by_kind: BTreeMap<String, u64> = SYMBOL_KIND_TOKENS
            .iter()
            .map(|token| (token.to_string(), 0))
            .collect();
        let mut stmt = conn
            .prepare_cached("SELECT kind, COUNT(*) FROM code_symbols GROUP BY kind")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(sql_err)?;
        for row in rows {
            let (kind, count) = row.map_err(sql_err)?;
            symbols_by_kind.insert(kind, count as u64);
        }

        let mut edges_by_kind: BTreeMap<String, u64> = EDGE_KIND_TOKENS
            .iter()
            .map(|token| (token.to_string(), 0))
            .collect();
        let mut stmt = conn
            .prepare_cached("SELECT kind, COUNT(*) FROM code_edges GROUP BY kind")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(sql_err)?;
        for row in rows {
            let (kind, count) = row.map_err(sql_err)?;
            edges_by_kind.insert(kind, count as u64);
        }

        Ok(StoreStats {
            schema_version: SCHEMA_VERSION,
            entities,
            symbols_by_kind,
            edges_by_kind,
        })
    }
}

impl Store for SqliteStore {
    fn put_source_ref(&self, source: &SourceRef) -> Result<(), StoreError> {
        let conn = self.lock()?;
        insert_source_ref(&conn, source)
    }

    fn get_source_ref(&self, source_id: &str) -> Result<Option<SourceRef>, StoreError> {
        let conn = self.lock()?;
        get_record_json(
            &conn,
            "SELECT record_json FROM source_refs WHERE source_id = ?1",
            source_id,
        )
    }

    fn put_memory_entry(&self, entry: &MemoryEntry) -> Result<(), StoreError> {
        let conn = self.lock()?;
        insert_memory_entry(&conn, entry)
    }

    fn get_memory_entry(&self, memory_entry_id: &str) -> Result<Option<MemoryEntry>, StoreError> {
        let conn = self.lock()?;
        get_record_json(
            &conn,
            "SELECT record_json FROM memory_entries WHERE memory_entry_id = ?1",
            memory_entry_id,
        )
    }

    fn put_provenance_record(&self, record: &ProvenanceRecord) -> Result<(), StoreError> {
        let conn = self.lock()?;
        insert_provenance_record(&conn, record)
    }

    fn get_provenance_record(
        &self,
        provenance_id: &str,
    ) -> Result<Option<ProvenanceRecord>, StoreError> {
        let conn = self.lock()?;
        get_record_json(
            &conn,
            "SELECT record_json FROM provenance_records WHERE provenance_id = ?1",
            provenance_id,
        )
    }

    fn put_event_log_entry(&self, event: &EventLogEntry) -> Result<(), StoreError> {
        let conn = self.lock()?;
        insert_event_log_entry(&conn, event)
    }

    fn get_event_log_entry(&self, event_id: &str) -> Result<Option<EventLogEntry>, StoreError> {
        let conn = self.lock()?;
        get_record_json(
            &conn,
            "SELECT record_json FROM event_log_entries WHERE event_id = ?1",
            event_id,
        )
    }

    fn put_code_map(&self, code_map: &CodeMap) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        let tx = guard.transaction().map_err(sql_err)?;
        insert_code_map(&tx, code_map)?;
        tx.commit().map_err(sql_err)
    }

    fn get_code_map(&self, code_map_id: &str) -> Result<Option<CodeMap>, StoreError> {
        let conn = self.lock()?;
        get_record_json(
            &conn,
            "SELECT record_json FROM code_maps WHERE code_map_id = ?1",
            code_map_id,
        )
    }

    fn lookup_source_refs_by_id(&self, source_id: &str) -> Result<Vec<SourceRef>, StoreError> {
        match self.get_source_ref(source_id)? {
            Some(source) => Ok(vec![source]),
            None => Ok(vec![]),
        }
    }

    fn lookup_source_refs_by_content_hash(
        &self,
        content_hash: &str,
    ) -> Result<Vec<SourceRef>, StoreError> {
        let conn = self.lock()?;
        collect_records(
            &conn,
            "SELECT record_json FROM source_refs WHERE content_hash = ?1 ORDER BY source_id",
            &[&content_hash],
        )
    }

    fn lookup_source_refs_by_origin_product(
        &self,
        origin_product: &str,
    ) -> Result<Vec<SourceRef>, StoreError> {
        let conn = self.lock()?;
        collect_records(
            &conn,
            "SELECT record_json FROM source_refs WHERE origin_product = ?1 ORDER BY source_id",
            &[&origin_product],
        )
    }

    fn lookup_source_refs_by_state(
        &self,
        state: &SourceState,
    ) -> Result<Vec<SourceRef>, StoreError> {
        let token = enum_token(state)?;
        let conn = self.lock()?;
        collect_records(
            &conn,
            "SELECT record_json FROM source_refs WHERE state = ?1 ORDER BY source_id",
            &[&token],
        )
    }

    fn lookup_source_refs_by_timestamp_range(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<SourceRef>, StoreError> {
        let start_time = OffsetDateTime::parse(start, &Rfc3339)
            .map_err(|e| StoreError::InvalidOperation(format!("invalid start timestamp: {}", e)))?;
        let end_time = OffsetDateTime::parse(end, &Rfc3339)
            .map_err(|e| StoreError::InvalidOperation(format!("invalid end timestamp: {}", e)))?;

        // RFC3339 strings with heterogeneous offsets do not sort
        // chronologically as text, so the comparison happens after parsing —
        // same semantics as `FileStore`.
        let conn = self.lock()?;
        let candidates: Vec<SourceRef> = collect_records(
            &conn,
            "SELECT record_json FROM source_refs ORDER BY source_id",
            &[],
        )?;

        Ok(candidates
            .into_iter()
            .filter(|source| {
                OffsetDateTime::parse(&source.created_at, &Rfc3339)
                    .map(|created| created >= start_time && created <= end_time)
                    .unwrap_or(false)
            })
            .collect())
    }

    fn lookup_memory_entries_by_state(
        &self,
        state: &IndexState,
    ) -> Result<Vec<MemoryEntry>, StoreError> {
        let token = enum_token(state)?;
        let conn = self.lock()?;
        collect_records(
            &conn,
            "SELECT record_json FROM memory_entries WHERE index_state = ?1 ORDER BY memory_entry_id",
            &[&token],
        )
    }

    fn list_all_provenance_records(&self) -> Result<Vec<ProvenanceRecord>, StoreError> {
        let conn = self.lock()?;
        collect_records(
            &conn,
            "SELECT record_json FROM provenance_records ORDER BY provenance_id",
            &[],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_token_tables_match_contract_enums() {
        for token in SYMBOL_KIND_TOKENS {
            symbol_kind_from_token(token).expect("symbol kind token parses");
        }
        for token in EDGE_KIND_TOKENS {
            edge_kind_from_token(token).expect("edge kind token parses");
        }
    }
}
