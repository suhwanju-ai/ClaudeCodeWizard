//! IMP-026 / PRD G-3. The decision is "the frontend gets no filesystem permission",
//! and an absence is silent — it cannot fail a build on its own. This test turns the
//! absence into an invariant CI enforces: adding `fs:allow-read` or `fs:allow-write`
//! breaks `cargo test`.

use std::path::Path;

#[test]
fn frontend_is_granted_no_filesystem_permission() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
    let raw = std::fs::read_to_string(&path).expect("capabilities/default.json must exist");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("capabilities/default.json must be valid JSON");

    let permissions = json["permissions"].as_array().expect("permissions must be an array");
    let permissions: Vec<&str> = permissions.iter().map(|p| p.as_str().expect("permission entries are strings")).collect();

    assert_eq!(
        permissions,
        ["core:default", "dialog:allow-open"],
        "the frontend must be granted no filesystem permission: every file access goes \
         through a Rust command (IMP-026). If you are adding a permission on purpose, \
         update TRD 9.4 and documents/system-architecture.md first."
    );
}
