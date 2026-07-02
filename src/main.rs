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

/// Format Unix seconds as RFC3339 timestamp.
/// Uses the `time` crate for correct date/time calculations,
/// accounting for leap years and variable month lengths.
fn format_rfc3339_from_unix_secs(secs_since_epoch: u64) -> String {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    match OffsetDateTime::from_unix_timestamp(secs_since_epoch as i64) {
        Ok(dt) => dt
            .format(&Rfc3339)
            .unwrap_or_else(|_| "INVALID".to_string()),
        Err(_) => "INVALID".to_string(),
    }
}

fn format_current_timestamp() -> String {
    // Return a valid RFC3339 timestamp for the current time.
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now();
    let secs_since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    format_rfc3339_from_unix_secs(secs_since_epoch)
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

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    fn format_rfc3339(secs_since_epoch: u64) -> String {
        // Convert Unix seconds to OffsetDateTime and format as RFC3339
        match OffsetDateTime::from_unix_timestamp(secs_since_epoch as i64) {
            Ok(dt) => dt
                .format(&Rfc3339)
                .unwrap_or_else(|_| "INVALID".to_string()),
            Err(_) => "INVALID".to_string(),
        }
    }

    #[test]
    fn test_format_rfc3339_validates_correctly() {
        // Test January 31, 1970 (a date that might produce invalid day-of-month)
        // Unix timestamp for 1970-01-31T00:00:00Z is 30 * 86400 = 2592000
        let jan_31_1970 = 30 * 86400;
        let timestamp = format_rfc3339(jan_31_1970);

        // Must parse as valid RFC3339
        let parse_result = OffsetDateTime::parse(&timestamp, &Rfc3339);
        assert!(
            parse_result.is_ok(),
            "2026-01-31 (timestamp {}) produced invalid RFC3339: '{}'",
            jan_31_1970,
            timestamp
        );

        // Test February 29, 2000 (leap year, naïve calculations might fail here)
        // 2000 is a leap year. Unix timestamp for 2000-02-29T00:00:00Z is 951868800
        let feb_29_2000_secs = 951868800u64;
        let timestamp = format_rfc3339(feb_29_2000_secs);

        let parse_result = OffsetDateTime::parse(&timestamp, &Rfc3339);
        assert!(
            parse_result.is_ok(),
            "2000-02-29 (timestamp {}) produced invalid RFC3339: '{}'",
            feb_29_2000_secs,
            timestamp
        );

        // Test March 1, 2000 (day after Feb 29 leap year)
        let mar_1_2000_secs = 951868800u64 + 86400;
        let timestamp = format_rfc3339(mar_1_2000_secs);

        let parse_result = OffsetDateTime::parse(&timestamp, &Rfc3339);
        assert!(
            parse_result.is_ok(),
            "2000-03-01 (timestamp {}) produced invalid RFC3339: '{}'",
            mar_1_2000_secs,
            timestamp
        );

        // Test a recent date (2026-06-30)
        // Unix timestamp for 2026-06-30T00:00:00Z is approximately 1777891200
        let date_2026_secs = 1777891200u64;
        let timestamp = format_rfc3339(date_2026_secs);

        let parse_result = OffsetDateTime::parse(&timestamp, &Rfc3339);
        assert!(
            parse_result.is_ok(),
            "2026-06-30 (timestamp {}) produced invalid RFC3339: '{}'",
            date_2026_secs,
            timestamp
        );
    }
}
