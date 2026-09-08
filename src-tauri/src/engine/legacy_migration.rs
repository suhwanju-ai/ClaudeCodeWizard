use std::fs;
use std::path::Path;

/// Sibling of `runs/` that pre-`resolvedStages` records are moved into.
pub const LEGACY_DIR_NAME: &str = "runs-legacy-v1";

/// Moves every run record that predates the required `resolvedStages` field out of
/// `runs_dir` and into a sibling `runs-legacy-v1/` directory, returning how many were
/// moved. Records that fail to parse at all are quarantined too, so a corrupt file can
/// never block startup (PRD D-5). Nothing is deleted (IMP-018 decision B / GAP-6).
pub fn quarantine_legacy_runs(runs_dir: &Path) -> Result<usize, std::io::Error> {
    if !runs_dir.exists() {
        return Ok(0);
    }
    let legacy_dir = runs_dir.with_file_name(LEGACY_DIR_NAME);
    let mut moved = 0usize;

    for entry in fs::read_dir(runs_dir)? {
        let path = entry?.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let is_legacy = match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(serde_json::Value::Object(map)) => !map.contains_key("resolvedStages"),
                _ => true,
            },
            Err(_) => true,
        };
        if !is_legacy {
            continue;
        }
        fs::create_dir_all(&legacy_dir)?;
        let file_name = path.file_name().expect("a file path has a file name").to_owned();
        fs::rename(&path, legacy_dir.join(file_name))?;
        moved += 1;
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::*;

    fn write(dir: &std::path::Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), body).unwrap();
    }

    const MODERN: &str = r#"{"runId":"r1","templateId":"t1","targetDir":"/tmp/x","status":"awaiting-stage-start","currentStageIndex":0,"stages":[],"resolvedStages":[]}"#;
    const LEGACY: &str = r#"{"runId":"r0","templateId":"t1","targetDir":"/tmp/x","status":"running","currentStageIndex":0,"stages":[]}"#;

    #[test]
    fn moves_records_without_resolved_stages() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "r0.json", LEGACY);

        let moved = quarantine_legacy_runs(&runs).unwrap();

        assert_eq!(moved, 1);
        assert!(!runs.join("r0.json").exists());
        assert!(root.path().join(LEGACY_DIR_NAME).join("r0.json").exists());
    }

    #[test]
    fn keeps_records_that_already_have_resolved_stages() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "r1.json", MODERN);

        let moved = quarantine_legacy_runs(&runs).unwrap();

        assert_eq!(moved, 0);
        assert!(runs.join("r1.json").exists());
        assert!(!root.path().join(LEGACY_DIR_NAME).exists());
    }

    #[test]
    fn moves_unparseable_records_too() {
        // PRD D-5: an unexplained deserialization failure must not block the app.
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "broken.json", "{ this is not json");

        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 1);
        assert!(root.path().join(LEGACY_DIR_NAME).join("broken.json").exists());
    }

    #[test]
    fn ignores_non_json_files() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "notes.txt", "hello");

        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 0);
        assert!(runs.join("notes.txt").exists());
    }

    #[test]
    fn is_a_noop_when_the_runs_dir_does_not_exist() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(quarantine_legacy_runs(&root.path().join("runs")).unwrap(), 0);
    }

    #[test]
    fn is_idempotent_across_two_startups() {
        let root = tempfile::tempdir().unwrap();
        let runs = root.path().join("runs");
        write(&runs, "r0.json", LEGACY);

        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 1);
        assert_eq!(quarantine_legacy_runs(&runs).unwrap(), 0);
    }
}
