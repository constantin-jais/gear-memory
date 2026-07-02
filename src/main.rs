use std::env;
use std::fs;
use std::path::Path;
use std::process;

use gear_memory::{FileStore, GearMemoryBundle, Store};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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

fn current_timestamp_rfc3339() -> Result<String, time::error::Format> {
    OffsetDateTime::now_utc().format(&Rfc3339)
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

    let timestamp = current_timestamp_rfc3339()?;

    store.mark_deleted(source_id, reason, &timestamp)?;

    Ok(())
}
