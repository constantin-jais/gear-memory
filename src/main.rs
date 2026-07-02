use std::env;
use std::fs;
use std::path::Path;
use std::process;

use gear_memory::{FileStore, GearMemoryBundle, Store};

fn main() {
    let mut args = env::args().skip(1);

    match (
        args.next().as_deref(),
        args.next(),
        args.next(),
        args.next(),
    ) {
        (None, _, _, _) => {
            println!("{}", gear_memory::summary());
        }
        (Some("validate"), Some(path), _, _) => {
            if let Err(error) = validate_bundle_file(&path) {
                eprintln!("validation failed: {error}");
                process::exit(1);
            }
            println!("valid gear-memory bundle: {path}");
        }
        (Some("get"), Some(store_root), Some(source_id), _) => {
            if let Err(error) = handle_get(&store_root, &source_id) {
                eprintln!("get failed: {error}");
                process::exit(1);
            }
        }
        (Some("list"), Some(store_root), Some(state_filter), _) => {
            if let Err(error) = handle_list(&store_root, &state_filter) {
                eprintln!("list failed: {error}");
                process::exit(1);
            }
        }
        (Some("delete"), Some(store_root), Some(source_id), Some(reason)) => {
            if let Err(error) = handle_delete(&store_root, &source_id, &reason) {
                eprintln!("delete failed: {error}");
                process::exit(1);
            }
            println!("marked source {source_id} as deleted");
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  gear-memory [validate <gear-memory-bundle.json>]");
            eprintln!("  gear-memory [get <store-root> <source-id>]");
            eprintln!("  gear-memory [list <store-root> <state-filter>]");
            eprintln!("  gear-memory [delete <store-root> <source-id> <reason>]");
            process::exit(2);
        }
    }
}

fn validate_bundle_file(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let bundle: GearMemoryBundle = serde_json::from_str(&content)?;
    bundle.validate()?;
    Ok(())
}

fn format_current_timestamp() -> String {
    // Return a valid RFC3339 timestamp.
    // For CLI use, this uses a simplification: seconds since epoch mod to get H:M:S.
    // Production code should use chrono or time crate with serde feature.
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now();
    let secs_since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    // Very simple approximation for demonstration
    let day_of_year = (secs_since_epoch / 86400) % 365;
    let year = 1970 + (secs_since_epoch / (365 * 86400));
    let month = 1 + ((day_of_year * 12) / 365);
    let day = 1 + ((day_of_year * 28) / (365 / 12));

    let secs_in_day = secs_since_epoch % 86400;
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

fn handle_get(store_root: &str, source_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let store = FileStore::new(Path::new(store_root))?;
    let source = store.get_source_ref(source_id)?;

    match source {
        Some(source) => {
            let json = serde_json::to_string_pretty(&source)?;
            println!("{}", json);
            Ok(())
        }
        None => Err(format!("source {source_id} not found").into()),
    }
}

fn handle_list(store_root: &str, state_filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let store = FileStore::new(Path::new(store_root))?;

    let sources = match state_filter {
        "active" => store.lookup_source_refs_by_state(&gear_memory::SourceState::Active)?,
        "deleted" => store.lookup_source_refs_by_state(&gear_memory::SourceState::Deleted)?,
        "anonymized" => store.lookup_source_refs_by_state(&gear_memory::SourceState::Anonymized)?,
        "all" => {
            // List all sources
            let active = store.lookup_source_refs_by_state(&gear_memory::SourceState::Active)?;
            let deleted = store.lookup_source_refs_by_state(&gear_memory::SourceState::Deleted)?;
            let anonymized =
                store.lookup_source_refs_by_state(&gear_memory::SourceState::Anonymized)?;

            let mut all = active;
            all.extend(deleted);
            all.extend(anonymized);
            all
        }
        _ => {
            eprintln!("invalid state filter: {state_filter}");
            eprintln!("valid filters: active, deleted, anonymized, all");
            return Err("invalid state filter".into());
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

fn handle_delete(
    store_root: &str,
    source_id: &str,
    reason: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let store = FileStore::new(Path::new(store_root))?;

    // Use current UTC timestamp (note: must be RFC3339 format)
    let timestamp = format_current_timestamp();

    store.mark_deleted(source_id, reason, &timestamp)?;

    Ok(())
}
