//! Bounded storage micro-benchmark for the gear-memory P1 engine decision
//! (annex of ADR 0002). Standalone binary, excluded from the gear-memory
//! dependency graph on purpose.
//!
//! Workload, fixed and deterministic (LCG seed 42):
//! - ingest 10 000 symbols + 20 000 edges,
//! - q1: count symbols whose name contains a fragment (leading wildcard —
//!   no index can help; measures scan/filter cost),
//! - q2: outgoing neighbors of one symbol (indexed point lookup),
//! - q3: BFS over outgoing edges, depth <= 3, same Rust algorithm for
//!   every engine.
//!
//! Cross-engine correctness: q1/q2/q3 results must be identical across
//! engines (asserted). Reported: ingest wall-clock, per-query median over
//! 100 iterations after 10 warmup runs, and on-disk size after ingest.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const N_SYMBOLS: u64 = 10_000;
const N_EDGES: usize = 20_000;
const ITERATIONS: usize = 100;
const WARMUP: usize = 10;
const SEARCH_FRAGMENT: &str = "store";
const BFS_DEPTH: usize = 3;

/// Deterministic probe: the symbol with the highest out-degree
/// (lexicographically smallest on ties), so q2/q3 exercise real traversal
/// whatever the generated edge distribution looks like.
fn probe_symbol(edges: &[Edge]) -> String {
    let mut out_degree: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for edge in edges {
        *out_degree.entry(edge.from.as_str()).or_default() += 1;
    }
    out_degree
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(id, _)| id.to_string())
        .expect("at least one edge")
}

struct Lcg(u64);

impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        // High bits: the low bits of a power-of-two-modulus LCG are periodic
        // (period 2^k for the low k bits) and bias small moduli.
        (self.next_u64() >> 32) % n
    }
}

struct Symbol {
    id: String,
    name: String,
    kind: String,
}

struct Edge {
    from: String,
    to: String,
}

fn dataset() -> (Vec<Symbol>, Vec<Edge>) {
    const KINDS: [&str; 5] = ["function", "type", "trait", "test", "module"];
    const STEMS: [&str; 8] = [
        "store",
        "parse",
        "index",
        "query",
        "graph",
        "bundle",
        "provenance",
        "symbol",
    ];

    let mut rng = Lcg(42);
    let symbols = (0..N_SYMBOLS)
        .map(|i| Symbol {
            id: format!("sym_{i:05}"),
            name: format!(
                "module{:03}::{}_{i:05}",
                i % 100,
                STEMS[rng.below(STEMS.len() as u64) as usize]
            ),
            kind: KINDS[rng.below(KINDS.len() as u64) as usize].to_string(),
        })
        .collect::<Vec<_>>();

    let mut edges = Vec::with_capacity(N_EDGES);
    while edges.len() < N_EDGES {
        let from = rng.below(N_SYMBOLS);
        // 30% of edges point into the first 200 symbols so the BFS probe
        // traverses a connected neighborhood instead of a sparse mist.
        let to = if rng.below(10) < 3 {
            rng.below(200)
        } else {
            rng.below(N_SYMBOLS)
        };
        if from != to {
            edges.push(Edge {
                from: format!("sym_{from:05}"),
                to: format!("sym_{to:05}"),
            });
        }
    }

    (symbols, edges)
}

trait Engine {
    fn label(&self) -> &'static str;
    fn ingest(&mut self, symbols: &[Symbol], edges: &[Edge]);
    fn count_symbols_matching(&self, fragment: &str) -> u64;
    fn neighbors_out(&self, id: &str) -> Vec<String>;
    fn db_size_bytes(&self) -> u64;
}

fn bfs(engine: &dyn Engine, start: &str, max_depth: usize) -> u64 {
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(start.to_string());
    let mut frontier = vec![start.to_string()];

    for _ in 0..max_depth {
        let mut next = Vec::new();
        for node in &frontier {
            for neighbor in engine.neighbors_out(node) {
                if visited.insert(neighbor.clone()) {
                    next.push(neighbor);
                }
            }
        }
        frontier = next;
    }

    visited.len() as u64
}

fn path_size_bytes(path: &Path) -> u64 {
    if path.is_dir() {
        fs::read_dir(path)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| path_size_bytes(&entry.path()))
                    .sum()
            })
            .unwrap_or(0)
    } else {
        fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    }
}

/// SQLite sidecar files (-wal, -shm) count toward the on-disk footprint.
fn sqlite_size_bytes(db_path: &Path) -> u64 {
    let mut total = path_size_bytes(db_path);
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = db_path.as_os_str().to_owned();
        sidecar.push(suffix);
        total += path_size_bytes(Path::new(&sidecar));
    }
    total
}

// --- rusqlite ---------------------------------------------------------------

struct SqliteEngine {
    conn: rusqlite::Connection,
    path: PathBuf,
}

impl SqliteEngine {
    fn new(dir: &Path) -> Self {
        let path = dir.join("bench.sqlite3");
        let conn = rusqlite::Connection::open(&path).expect("open sqlite");
        conn.pragma_update(None, "journal_mode", "WAL")
            .expect("WAL");
        conn.pragma_update(None, "synchronous", "NORMAL")
            .expect("synchronous");
        Self { conn, path }
    }
}

impl Engine for SqliteEngine {
    fn label(&self) -> &'static str {
        "rusqlite 0.40 (bundled, WAL)"
    }

    fn ingest(&mut self, symbols: &[Symbol], edges: &[Edge]) {
        self.conn
            .execute_batch(
                "CREATE TABLE symbols (id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL);
                 CREATE TABLE edges (from_id TEXT NOT NULL, to_id TEXT NOT NULL);",
            )
            .expect("schema");

        let tx = self.conn.transaction().expect("begin");
        {
            let mut sym = tx
                .prepare("INSERT INTO symbols VALUES (?1, ?2, ?3)")
                .expect("prepare symbols");
            for s in symbols {
                sym.execute(rusqlite::params![s.id, s.name, s.kind])
                    .expect("insert symbol");
            }
            let mut edg = tx
                .prepare("INSERT INTO edges VALUES (?1, ?2)")
                .expect("prepare edges");
            for e in edges {
                edg.execute(rusqlite::params![e.from, e.to])
                    .expect("insert edge");
            }
        }
        tx.commit().expect("commit");

        // Deferred index creation, per the reference design's flush phase.
        self.conn
            .execute_batch(
                "CREATE INDEX idx_symbols_name ON symbols(name);
                 CREATE INDEX idx_edges_from ON edges(from_id);",
            )
            .expect("indexes");
    }

    fn count_symbols_matching(&self, fragment: &str) -> u64 {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM symbols WHERE name LIKE '%' || ?1 || '%'",
                rusqlite::params![fragment],
                |row| row.get(0),
            )
            .expect("count");
        count as u64
    }

    fn neighbors_out(&self, id: &str) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT to_id FROM edges WHERE from_id = ?1 ORDER BY to_id")
            .expect("prepare neighbors");
        let rows = stmt
            .query_map(rusqlite::params![id], |row| row.get::<_, String>(0))
            .expect("query neighbors");
        rows.collect::<Result<Vec<_>, _>>().expect("neighbor rows")
    }

    fn db_size_bytes(&self) -> u64 {
        sqlite_size_bytes(&self.path)
    }
}

// --- redb -------------------------------------------------------------------

const SYMBOLS_TABLE: redb::TableDefinition<&str, (&str, &str)> =
    redb::TableDefinition::new("symbols");
const EDGES_TABLE: redb::TableDefinition<(&str, &str), ()> = redb::TableDefinition::new("edges");

struct RedbEngine {
    db: redb::Database,
    path: PathBuf,
}

impl RedbEngine {
    fn new(dir: &Path) -> Self {
        let path = dir.join("bench.redb");
        let db = redb::Database::create(&path).expect("create redb");
        Self { db, path }
    }
}

impl Engine for RedbEngine {
    fn label(&self) -> &'static str {
        "redb 4.1 (pure Rust)"
    }

    fn ingest(&mut self, symbols: &[Symbol], edges: &[Edge]) {
        let write_txn = self.db.begin_write().expect("begin write");
        {
            let mut table = write_txn.open_table(SYMBOLS_TABLE).expect("open symbols");
            for s in symbols {
                table
                    .insert(s.id.as_str(), (s.name.as_str(), s.kind.as_str()))
                    .expect("insert symbol");
            }
            let mut table = write_txn.open_table(EDGES_TABLE).expect("open edges");
            for e in edges {
                table
                    .insert((e.from.as_str(), e.to.as_str()), ())
                    .expect("insert edge");
            }
        }
        write_txn.commit().expect("commit");
    }

    fn count_symbols_matching(&self, fragment: &str) -> u64 {
        use redb::{ReadableDatabase, ReadableTable};

        let read_txn = self.db.begin_read().expect("begin read");
        let table = read_txn.open_table(SYMBOLS_TABLE).expect("open symbols");
        let mut count = 0;
        for entry in table.iter().expect("iter symbols") {
            let (_, value) = entry.expect("symbol entry");
            if value.value().0.contains(fragment) {
                count += 1;
            }
        }
        count
    }

    fn neighbors_out(&self, id: &str) -> Vec<String> {
        use redb::ReadableDatabase;

        let read_txn = self.db.begin_read().expect("begin read");
        let table = read_txn.open_table(EDGES_TABLE).expect("open edges");
        // Prefix scan over the composite key; '\u{10FFFF}' is above any
        // character used by our ASCII identifiers.
        let range = table
            .range((id, "")..=(id, "\u{10FFFF}"))
            .expect("range edges");
        range
            .map(|entry| entry.expect("edge entry").0.value().1.to_string())
            .collect()
    }

    fn db_size_bytes(&self) -> u64 {
        path_size_bytes(&self.path)
    }
}

// --- stoolap ------------------------------------------------------------------

struct StoolapEngine {
    db: stoolap::api::Database,
    path: PathBuf,
}

impl StoolapEngine {
    fn new(dir: &Path) -> Self {
        let path = dir.join("bench-stoolap");
        let dsn = format!("file://{}", path.display());
        let db = stoolap::api::Database::open(&dsn).expect("open stoolap");
        Self { db, path }
    }
}

impl Engine for StoolapEngine {
    fn label(&self) -> &'static str {
        "stoolap 0.4 (pure Rust SQL)"
    }

    fn ingest(&mut self, symbols: &[Symbol], edges: &[Edge]) {
        // stoolap 0.4 limitation found empirically: only INTEGER PRIMARY KEY
        // is supported, so the TEXT id gets a deferred UNIQUE index instead.
        self.db
            .execute("CREATE TABLE symbols (id TEXT, name TEXT, kind TEXT)", ())
            .expect("schema symbols");
        self.db
            .execute("CREATE TABLE edges (from_id TEXT, to_id TEXT)", ())
            .expect("schema edges");

        // Prepared-statement loop is the crate's own benchmark idiom
        // (examples/benchmark.rs); no explicit transaction API is exposed.
        let sym = self
            .db
            .prepare("INSERT INTO symbols VALUES ($1, $2, $3)")
            .expect("prepare symbols");
        for s in symbols {
            sym.execute((s.id.as_str(), s.name.as_str(), s.kind.as_str()))
                .expect("insert symbol");
        }
        let edg = self
            .db
            .prepare("INSERT INTO edges VALUES ($1, $2)")
            .expect("prepare edges");
        for e in edges {
            edg.execute((e.from.as_str(), e.to.as_str()))
                .expect("insert edge");
        }

        self.db
            .execute("CREATE UNIQUE INDEX uidx_symbols_id ON symbols(id)", ())
            .expect("unique index symbols");
        self.db
            .execute("CREATE INDEX idx_edges_from ON edges(from_id)", ())
            .expect("index edges");
    }

    fn count_symbols_matching(&self, fragment: &str) -> u64 {
        let pattern = format!("%{fragment}%");
        let count: i64 = self
            .db
            .query_one(
                "SELECT COUNT(*) FROM symbols WHERE name LIKE $1",
                (pattern.as_str(),),
            )
            .expect("count");
        count as u64
    }

    fn neighbors_out(&self, id: &str) -> Vec<String> {
        let rows = self
            .db
            .query(
                "SELECT to_id FROM edges WHERE from_id = $1 ORDER BY to_id",
                (id,),
            )
            .expect("query neighbors");
        rows.map(|row| {
            row.expect("neighbor row")
                .get::<String>(0)
                .expect("to_id column")
        })
        .collect()
    }

    fn db_size_bytes(&self) -> u64 {
        path_size_bytes(&self.path)
    }
}

// --- harness ------------------------------------------------------------------

struct Report {
    label: &'static str,
    ingest_ms: f64,
    q1_us: f64,
    q2_us: f64,
    q3_us: f64,
    size_kib: f64,
    q1_count: u64,
    q2_count: u64,
    q3_visited: u64,
}

fn median_micros(mut samples: Vec<u128>) -> f64 {
    samples.sort_unstable();
    samples[samples.len() / 2] as f64 / 1_000.0
}

fn time_query<R>(mut run: impl FnMut() -> R) -> (f64, R) {
    for _ in 0..WARMUP {
        run();
    }
    let mut samples = Vec::with_capacity(ITERATIONS);
    let mut last = None;
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let result = run();
        samples.push(start.elapsed().as_nanos());
        last = Some(result);
    }
    (median_micros(samples), last.expect("at least one run"))
}

fn run_engine(engine: &mut dyn Engine, symbols: &[Symbol], edges: &[Edge], probe: &str) -> Report {
    let start = Instant::now();
    engine.ingest(symbols, edges);
    let ingest_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let (q1_us, q1_count) = time_query(|| engine.count_symbols_matching(SEARCH_FRAGMENT));
    let (q2_us, q2_neighbors) = time_query(|| engine.neighbors_out(probe));
    let engine_ref: &dyn Engine = engine;
    let (q3_us, q3_visited) = time_query(|| bfs(engine_ref, probe, BFS_DEPTH));

    Report {
        label: engine.label(),
        ingest_ms,
        q1_us,
        q2_us,
        q3_us,
        size_kib: engine.db_size_bytes() as f64 / 1024.0,
        q1_count,
        q2_count: q2_neighbors.len() as u64,
        q3_visited,
    }
}

fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gm-storage-bench-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create bench dir");
    dir
}

fn main() {
    let (symbols, edges) = dataset();
    let probe = probe_symbol(&edges);
    println!(
        "workload: {} symbols, {} edges, probe {} (max out-degree), {} iterations (median), {} warmup\n",
        symbols.len(),
        edges.len(),
        probe,
        ITERATIONS,
        WARMUP
    );

    let mut reports = Vec::new();

    let mut sqlite = SqliteEngine::new(&fresh_dir("sqlite"));
    reports.push(run_engine(&mut sqlite, &symbols, &edges, &probe));

    let mut redb_engine = RedbEngine::new(&fresh_dir("redb"));
    reports.push(run_engine(&mut redb_engine, &symbols, &edges, &probe));

    let mut stoolap_engine = StoolapEngine::new(&fresh_dir("stoolap"));
    reports.push(run_engine(&mut stoolap_engine, &symbols, &edges, &probe));

    // Cross-engine correctness: every engine must agree on every result.
    for pair in reports.windows(2) {
        assert_eq!(
            pair[0].q1_count, pair[1].q1_count,
            "q1 mismatch: {} vs {}",
            pair[0].label, pair[1].label
        );
        assert_eq!(
            pair[0].q2_count, pair[1].q2_count,
            "q2 mismatch: {} vs {}",
            pair[0].label, pair[1].label
        );
        assert_eq!(
            pair[0].q3_visited, pair[1].q3_visited,
            "q3 mismatch: {} vs {}",
            pair[0].label, pair[1].label
        );
    }

    println!(
        "correctness (identical across engines): q1={} symbols matched, q2={} neighbors, q3={} nodes visited\n",
        reports[0].q1_count, reports[0].q2_count, reports[0].q3_visited
    );
    println!(
        "| engine | ingest (ms) | q1 search (µs) | q2 neighbors (µs) | q3 BFS≤3 (µs) | size (KiB) |"
    );
    println!("| --- | ---: | ---: | ---: | ---: | ---: |");
    for r in &reports {
        println!(
            "| {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.0} |",
            r.label, r.ingest_ms, r.q1_us, r.q2_us, r.q3_us, r.size_kib
        );
    }
}
