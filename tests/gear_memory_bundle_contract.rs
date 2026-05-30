use std::fs;
use std::process::Command;

use gear_memory::GearMemoryBundle;

#[test]
fn minimal_p0_bundle_fixture_is_valid() {
    let payload = fs::read_to_string("tests/fixtures/gear-memory-minimal.valid.json")
        .expect("fixture is readable");
    let bundle: GearMemoryBundle = serde_json::from_str(&payload).expect("fixture deserializes");

    bundle.validate().expect("fixture validates");
}

#[test]
fn secret_metadata_fixture_is_rejected() {
    let payload = fs::read_to_string("tests/fixtures/gear-memory-secret-metadata.invalid.json")
        .expect("fixture is readable");
    let bundle: GearMemoryBundle = serde_json::from_str(&payload).expect("fixture deserializes");

    let error = bundle.validate().expect_err("unsafe metadata is rejected");

    assert!(error.to_string().contains("api_key"));
}

#[test]
fn cli_validate_accepts_minimal_fixture() {
    let binary = env!("CARGO_BIN_EXE_gear-memory");
    let output = Command::new(binary)
        .args(["validate", "tests/fixtures/gear-memory-minimal.valid.json"])
        .output()
        .expect("CLI runs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("valid gear-memory bundle"));
}
