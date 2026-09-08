use std::path::{Path, PathBuf};

use crate::template::Template;

use super::run_record::RunRecord;

/// The directory this app creates inside every target project folder.
pub const MANIFEST_DIR_NAME: &str = ".claude-pipeline-wizard";

#[derive(Debug, thiserror::Error)]
pub enum ProjectManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum TargetDirError {
    #[error("target dir must be an absolute path: {0}")]
    NotAbsolute(String),
    #[error("target dir is not a directory: {0}")]
    NotADirectory(String),
    #[error("io error while checking target dir: {0}")]
    Io(#[from] std::io::Error),
}

pub fn manifest_dir(target_dir: &Path) -> PathBuf {
    target_dir.join(MANIFEST_DIR_NAME)
}

pub fn manifest_run_path(target_dir: &Path) -> PathBuf {
    manifest_dir(target_dir).join("run.json")
}

/// IMP-019 (decision D6). Requires an absolute path — a relative one would resolve
/// against the app's working directory, which the user never sees — then creates the
/// folder if needed and canonicalizes it *only* to confirm it is really a directory
/// once symlinks are resolved.
///
/// The canonical form is deliberately discarded: on Windows `canonicalize` returns a
/// `\\?\` UNC path, and `RunRecord.target_dir` is shown verbatim in the UI. The caller
/// keeps its original path for storage and display.
pub fn validate_target_dir(target_dir: &Path) -> Result<(), TargetDirError> {
    if !target_dir.is_absolute() {
        return Err(TargetDirError::NotAbsolute(target_dir.to_string_lossy().to_string()));
    }
    std::fs::create_dir_all(target_dir)?;
    let canonical = std::fs::canonicalize(target_dir)?;
    if !canonical.is_dir() {
        return Err(TargetDirError::NotADirectory(canonical.to_string_lossy().to_string()));
    }
    Ok(())
}

/// Writes the two manifest files. Called on every state transition, so the folder always
/// reflects the run's current state (PRD B-1).
///
/// `run.json` is the whole `RunRecord`, `resolvedStages` included. `pipeline.json` is a
/// `Template`-shaped snapshot of `resolved_stages` — the definitions this run is actually
/// using, not whatever the gallery template says now.
pub fn write_project_manifest(target_dir: &Path, record: &RunRecord) -> Result<(), ProjectManifestError> {
    let dir = manifest_dir(target_dir);
    std::fs::create_dir_all(&dir)?;

    std::fs::write(dir.join("run.json"), serde_json::to_string_pretty(record)?)?;

    // M4: RunRecord carries no template name/description, so the id is reused as the
    // name and the description is left empty.
    let snapshot = Template {
        id: record.template_id.clone(),
        name: record.template_id.clone(),
        description: String::new(),
        stages: record.resolved_stages.clone(),
    };
    std::fs::write(dir.join("pipeline.json"), serde_json::to_string_pretty(&snapshot)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    fn stage(id: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: format!("Stage {id}"),
            prompt: format!("prompt for {id}"),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec!["Read".to_string()],
            checkpoint: true,
        }
    }

    fn record(target_dir: &Path) -> RunRecord {
        RunRecord::new(
            "run1".to_string(),
            "tpl1".to_string(),
            target_dir.to_string_lossy().to_string(),
            &[stage("a"), stage("b")],
        )
    }

    #[test]
    fn manifest_paths_are_under_the_fixed_directory_name() {
        let dir = Path::new("/tmp/proj");
        assert_eq!(manifest_dir(dir), dir.join(".claude-pipeline-wizard"));
        assert_eq!(manifest_run_path(dir), manifest_dir(dir).join("run.json"));
    }

    #[test]
    fn writes_both_files_with_the_expected_shape() {
        let target = tempfile::tempdir().unwrap();
        let rec = record(target.path());

        write_project_manifest(target.path(), &rec).unwrap();

        let run_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(manifest_run_path(target.path())).unwrap()).unwrap();
        assert_eq!(run_json["runId"], "run1");
        assert_eq!(run_json["status"], "awaiting-stage-start");
        assert_eq!(run_json["resolvedStages"].as_array().unwrap().len(), 2);

        let pipeline_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_dir(target.path()).join("pipeline.json")).unwrap(),
        )
        .unwrap();
        // M4: RunRecord has no template name/description, so the id is reused for the
        // name and the description is left empty.
        assert_eq!(pipeline_json["id"], "tpl1");
        assert_eq!(pipeline_json["name"], "tpl1");
        assert_eq!(pipeline_json["description"], "");
        assert_eq!(pipeline_json["stages"].as_array().unwrap().len(), 2);
        assert_eq!(pipeline_json["stages"][0]["id"], "a");
    }

    #[test]
    fn pipeline_json_snapshots_resolved_stages_not_the_original() {
        let target = tempfile::tempdir().unwrap();
        let mut rec = record(target.path());
        let mut edited = stage("a");
        edited.prompt = "edited for this run only".to_string();
        rec.apply_stage_override(edited).unwrap();

        write_project_manifest(target.path(), &rec).unwrap();

        let pipeline_json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_dir(target.path()).join("pipeline.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(pipeline_json["stages"][0]["prompt"], "edited for this run only");
    }

    #[test]
    fn rewriting_replaces_the_previous_content() {
        let target = tempfile::tempdir().unwrap();
        let mut rec = record(target.path());
        write_project_manifest(target.path(), &rec).unwrap();
        let before = std::fs::read_to_string(manifest_run_path(target.path())).unwrap();

        rec.begin_current_stage();
        write_project_manifest(target.path(), &rec).unwrap();
        let after = std::fs::read_to_string(manifest_run_path(target.path())).unwrap();

        assert_ne!(before, after);
        assert!(after.contains("\"running\""));
    }

    #[test]
    fn write_fails_when_the_manifest_dir_slot_is_occupied_by_a_file() {
        let target = tempfile::tempdir().unwrap();
        std::fs::write(manifest_dir(target.path()), "not a directory").unwrap();
        assert!(matches!(
            write_project_manifest(target.path(), &record(target.path())),
            Err(ProjectManifestError::Io(_))
        ));
    }

    #[test]
    fn validate_target_dir_rejects_a_relative_path() {
        assert!(matches!(
            validate_target_dir(Path::new("relative/path")),
            Err(TargetDirError::NotAbsolute(_))
        ));
    }

    #[test]
    fn validate_target_dir_accepts_and_creates_a_missing_absolute_dir() {
        let root = tempfile::tempdir().unwrap();
        let fresh = root.path().join("brand-new-project");
        assert!(!fresh.exists());

        validate_target_dir(&fresh).unwrap();

        assert!(fresh.is_dir(), "an absolute path that does not exist yet must still be creatable");
    }

    #[test]
    fn validate_target_dir_rejects_a_path_that_is_a_file() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("a-file.txt");
        std::fs::write(&file, "x").unwrap();

        assert!(matches!(validate_target_dir(&file), Err(TargetDirError::Io(_))));
    }
}
