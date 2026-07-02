use std::env;
use std::fs;
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use gear_memory::{
    FileStore, GearMemoryBundle, ProvenanceOperation, ProvenanceRecord, SafeMetadata, SqliteStore,
    Store,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Per-repo default, git-ignored; explicit `--db` for any other location.
/// No hidden global cache (decision log, 2026-07-02).
const DEFAULT_DB_PATH: &str = "./.gear-memory/db.sqlite3";

const API_VERSION: &str = "gear.memory.v0.1";

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        println!("{}", gear_memory::summary());
        return;
    }

    let command = args.remove(0);
    let outcome = match command.as_str() {
        "validate" => cmd_validate(&args),
        "get" => cmd_get(&args),
        "list" => cmd_list(&args),
        "delete" => cmd_delete(&args),
        "ingest" => cmd_ingest(args),
        "query" => cmd_query(args),
        "trace" => cmd_trace(args),
        "stats" => cmd_stats(args),
        "snippet" => cmd_snippet(args),
        _ => {
            usage();
            process::exit(2);
        }
    };

    if let Err(error) = outcome {
        eprintln!("{command} failed: {error}");
        process::exit(1);
    }
}

fn usage() {
    eprintln!("usage:");
    eprintln!("  gear-memory validate <gear-memory-bundle.json>");
    eprintln!("  gear-memory get <store-root> <source-id>");
    eprintln!("  gear-memory list <store-root> <state-filter>");
    eprintln!("  gear-memory delete <store-root> <source-id> <reason>");
    eprintln!("  gear-memory ingest <gear-memory-bundle.json> [--db <path>]");
    eprintln!("  gear-memory query symbols --name <substring> [--kind <kind>] [--db <path>]");
    eprintln!("  gear-memory trace <code-map-id> <symbol-id> [--depth <n>] [--db <path>]");
    eprintln!("  gear-memory stats [--db <path>]");
    eprintln!("  gear-memory snippet <code-map-id> <symbol-id> [--db <path>]");
    eprintln!("default --db: {DEFAULT_DB_PATH}");
}

/// Remove `flag` and its value from `args`, if present.
fn take_flag(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, String> {
    match args.iter().position(|arg| arg == flag) {
        None => Ok(None),
        Some(position) => {
            args.remove(position);
            if position < args.len() {
                Ok(Some(args.remove(position)))
            } else {
                Err(format!("{flag} expects a value"))
            }
        }
    }
}

fn take_db_path(args: &mut Vec<String>) -> Result<String, String> {
    Ok(take_flag(args, "--db")?.unwrap_or_else(|| DEFAULT_DB_PATH.to_string()))
}

fn print_envelope(
    data: serde_json::Value,
    meta: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let envelope = json!({ "data": data, "meta": meta });
    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
}

fn cmd_validate(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = args
        .first()
        .ok_or("usage: gear-memory validate <gear-memory-bundle.json>")?;

    let content = fs::read_to_string(path)?;
    let bundle: GearMemoryBundle = serde_json::from_str(&content)?;
    bundle.validate()?;

    println!("valid gear-memory bundle: {path}");
    Ok(())
}

fn cmd_get(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (store_root, source_id) = match args {
        [root, id, ..] => (root, id),
        _ => return Err("usage: gear-memory get <store-root> <source-id>".into()),
    };

    let store = FileStore::new(Path::new(store_root))?;
    match store.get_source_ref(source_id)? {
        Some(source) => {
            println!("{}", serde_json::to_string_pretty(&source)?);
            Ok(())
        }
        None => Err(format!("source {source_id} not found").into()),
    }
}

fn cmd_list(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (store_root, state_filter) = match args {
        [root, state, ..] => (root, state),
        _ => return Err("usage: gear-memory list <store-root> <state-filter>".into()),
    };

    let store = FileStore::new(Path::new(store_root))?;
    let sources = match state_filter.as_str() {
        "active" => store.lookup_source_refs_by_state(&gear_memory::SourceState::Active)?,
        "deleted" => store.lookup_source_refs_by_state(&gear_memory::SourceState::Deleted)?,
        "anonymized" => store.lookup_source_refs_by_state(&gear_memory::SourceState::Anonymized)?,
        "all" => {
            let mut all = store.lookup_source_refs_by_state(&gear_memory::SourceState::Active)?;
            all.extend(store.lookup_source_refs_by_state(&gear_memory::SourceState::Deleted)?);
            all.extend(store.lookup_source_refs_by_state(&gear_memory::SourceState::Anonymized)?);
            all
        }
        other => {
            return Err(
                format!("invalid state filter `{other}` (active|deleted|anonymized|all)").into(),
            );
        }
    };

    if sources.is_empty() {
        println!("no sources found");
    } else {
        for source in sources {
            println!(
                "{:20} | {:20} | {:15} | {}",
                source.source_id,
                format!("{:?}", source.state),
                source.origin_product,
                source.created_at
            );
        }
    }

    Ok(())
}

fn cmd_delete(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (store_root, source_id, reason) = match args {
        [root, id, reason, ..] => (root, id, reason),
        _ => return Err("usage: gear-memory delete <store-root> <source-id> <reason>".into()),
    };

    let store = FileStore::new(Path::new(store_root))?;
    let timestamp = current_timestamp_rfc3339()?;
    store.mark_deleted(source_id, reason, &timestamp)?;

    println!("marked source {source_id} as deleted");
    Ok(())
}

fn cmd_ingest(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = take_db_path(&mut args)?;
    let bundle_path = args
        .first()
        .ok_or("usage: gear-memory ingest <gear-memory-bundle.json> [--db <path>]")?
        .clone();

    let content = fs::read_to_string(&bundle_path)?;
    let bundle: GearMemoryBundle = serde_json::from_str(&content)?;

    // Deterministic ingestion identity: the same bundle content always maps
    // to the same provenance id, so re-ingestion stays idempotent.
    let digest = Sha256::digest(content.as_bytes());
    let content_hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();

    let mut outputs: Vec<String> = bundle
        .code_maps
        .iter()
        .map(|code_map| code_map.code_map_id.clone())
        .chain(bundle.source_refs.iter().map(|s| s.source_id.clone()))
        .collect();
    if outputs.is_empty() {
        outputs.push(db_path.clone());
    }

    let provenance = ProvenanceRecord {
        provenance_id: format!("prov_ingest_{}", &content_hex[..16]),
        actor_ref: "gear-memory:cli".to_string(),
        operation: ProvenanceOperation::Indexed,
        inputs: vec![bundle_path.clone()],
        outputs,
        tool_ref: Some(format!("gear-memory@{}", env!("CARGO_PKG_VERSION"))),
        timestamp: current_timestamp_rfc3339()?,
        metadata: SafeMetadata::default(),
    };

    let store = SqliteStore::new(Path::new(&db_path))?;
    let report = store.ingest_bundle(&bundle, &provenance)?;

    print_envelope(
        json!({ "report": report, "provenance_id": provenance.provenance_id }),
        json!({ "api": API_VERSION, "command": "ingest", "db": db_path, "bundle": bundle_path }),
    )
}

fn cmd_query(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = take_db_path(&mut args)?;
    let name = take_flag(&mut args, "--name")?.ok_or(
        "usage: gear-memory query symbols --name <substring> [--kind <kind>] [--db <path>]",
    )?;
    let kind_token = take_flag(&mut args, "--kind")?;

    match args.first().map(String::as_str) {
        Some("symbols") => {}
        _ => return Err("only `query symbols` is supported".into()),
    }

    let kind = kind_token
        .map(|token| {
            serde_json::from_value::<gear_memory::CodeSymbolKind>(serde_json::Value::String(
                token.clone(),
            ))
            .map_err(|_| format!("unknown symbol kind `{token}`"))
        })
        .transpose()?;

    let store = SqliteStore::new(Path::new(&db_path))?;
    let hits = store.symbol_search(&name, kind.as_ref())?;

    let symbols: Vec<serde_json::Value> = hits
        .iter()
        .map(|(code_map_id, symbol)| json!({ "code_map_id": code_map_id, "symbol": symbol }))
        .collect();

    print_envelope(
        json!({ "symbols": symbols }),
        json!({
            "api": API_VERSION,
            "command": "query symbols",
            "count": hits.len(),
            "db": db_path,
            "truncated": false,
        }),
    )
}

fn cmd_trace(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = take_db_path(&mut args)?;
    let max_depth: u32 = match take_flag(&mut args, "--depth")? {
        Some(raw) => raw
            .parse()
            .map_err(|_| format!("--depth expects an integer, got `{raw}`"))?,
        None => 3,
    };

    let (code_map_id, symbol_id) = match args.as_slice() {
        [code_map_id, symbol_id, ..] => (code_map_id, symbol_id),
        _ => {
            return Err(
                "usage: gear-memory trace <code-map-id> <symbol-id> [--depth <n>] [--db <path>]"
                    .into(),
            );
        }
    };

    let store = SqliteStore::new(Path::new(&db_path))?;
    let hops = store.trace_bfs(code_map_id, symbol_id, max_depth)?;

    print_envelope(
        json!({ "hops": hops }),
        json!({
            "api": API_VERSION,
            "command": "trace",
            "code_map_id": code_map_id,
            "count": hops.len(),
            "db": db_path,
            "max_depth": max_depth,
        }),
    )
}

fn cmd_stats(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = take_db_path(&mut args)?;

    let store = SqliteStore::new(Path::new(&db_path))?;
    let stats = store.stats()?;

    print_envelope(
        serde_json::to_value(&stats)?,
        json!({ "api": API_VERSION, "command": "stats", "db": db_path }),
    )
}

fn cmd_snippet(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = take_db_path(&mut args)?;
    let (code_map_id, symbol_id) = match args.as_slice() {
        [code_map_id, symbol_id, ..] => (code_map_id, symbol_id),
        _ => {
            return Err(
                "usage: gear-memory snippet <code-map-id> <symbol-id> [--db <path>]".into(),
            );
        }
    };

    let store = SqliteStore::new(Path::new(&db_path))?;
    let code_map = store
        .get_code_map(code_map_id)?
        .ok_or_else(|| format!("code map {code_map_id} not found"))?;
    let symbol = code_map
        .symbols
        .iter()
        .find(|candidate| &candidate.symbol_id == symbol_id)
        .ok_or_else(|| format!("symbol {symbol_id} not found in {code_map_id}"))?;

    let source = store.get_source_ref(&symbol.source_ref)?;
    let source_uri = source.as_ref().and_then(|s| s.uri.clone());

    // Best effort: the substrate stores references, not source truth. The
    // snippet is only read back when the URI points at a readable local file.
    let snippet = source_uri
        .as_deref()
        .and_then(|uri| uri.strip_prefix("file://"))
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|content| {
            let start = symbol.range.start_line.saturating_sub(1) as usize;
            let len = (symbol.range.end_line - symbol.range.start_line + 1) as usize;
            content
                .lines()
                .skip(start)
                .take(len)
                .collect::<Vec<_>>()
                .join("\n")
        });

    let note = if snippet.is_none() {
        Some("source content unavailable locally; returning the reference only")
    } else {
        None
    };

    print_envelope(
        json!({ "symbol": symbol, "source_uri": source_uri, "snippet": snippet }),
        json!({
            "api": API_VERSION,
            "command": "snippet",
            "db": db_path,
            "note": note,
        }),
    )
}

fn rfc3339_from_unix_secs(secs: i64) -> Result<String, Box<dyn std::error::Error>> {
    let timestamp = OffsetDateTime::from_unix_timestamp(secs)?;
    Ok(timestamp.format(&Rfc3339)?)
}

fn current_timestamp_rfc3339() -> Result<String, Box<dyn std::error::Error>> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    rfc3339_from_unix_secs(i64::try_from(secs)?)
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    use super::rfc3339_from_unix_secs;

    #[test]
    fn epoch_boundaries_format_exactly() {
        assert_eq!(
            rfc3339_from_unix_secs(0).expect("format epoch"),
            "1970-01-01T00:00:00Z"
        );
        // Day 31: the removed hand-rolled formula mapped this date to January 29.
        assert_eq!(
            rfc3339_from_unix_secs(2_592_000).expect("format day 31"),
            "1970-01-31T00:00:00Z"
        );
        // Leap day: unrepresentable in a 365-day approximation.
        assert_eq!(
            rfc3339_from_unix_secs(951_782_400).expect("format leap day"),
            "2000-02-29T00:00:00Z"
        );
    }

    #[test]
    fn every_day_of_a_leap_and_a_common_year_parses_as_rfc3339() {
        // 2024-01-01T00:00:00Z and 2026-01-01T00:00:00Z, one timestamp per day.
        for year_start in [1_704_067_200_i64, 1_767_225_600] {
            for day in 0..366 {
                let formatted =
                    rfc3339_from_unix_secs(year_start + day * 86_400).expect("format day");
                OffsetDateTime::parse(&formatted, &Rfc3339).expect("parses as RFC3339");
            }
        }
    }
}
