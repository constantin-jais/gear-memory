use std::env;
use std::fs;
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn rfc3339_from_unix_secs(secs: i64) -> Result<String, Box<dyn std::error::Error>> {
    let timestamp = OffsetDateTime::from_unix_timestamp(secs)?;
    Ok(timestamp.format(&Rfc3339)?)
}

fn current_timestamp_rfc3339() -> Result<String, Box<dyn std::error::Error>> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    rfc3339_from_unix_secs(i64::try_from(secs)?)
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
