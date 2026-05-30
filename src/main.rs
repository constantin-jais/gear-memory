use std::env;
use std::fs;
use std::process;

use gear_memory::GearMemoryBundle;

fn main() {
    let mut args = env::args().skip(1);

    match (args.next().as_deref(), args.next()) {
        (None, _) => {
            println!("{}", gear_memory::summary());
        }
        (Some("validate"), Some(path)) => {
            if let Err(error) = validate_bundle_file(&path) {
                eprintln!("validation failed: {error}");
                process::exit(1);
            }
            println!("valid gear-memory bundle: {path}");
        }
        _ => {
            eprintln!("usage: gear-memory [validate <gear-memory-bundle.json>]");
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
