use std::fs;
use std::path::{Path, PathBuf};

use crate::template::Template;

use super::run_record::RunRecord;

#[derive(Debug, thiserror::Error)]
pub enum ProjectManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn manifest_dir(target_dir: &Path) -> PathBuf {
    target_dir.join(".claude-pipeline-wizard")
}

/// Writes the run's live state (`run.json`) and a resolved-template
/// snapshot (`pipeline.json`, reflecting any per-run stage edits) into a
/// hidden directory inside the run's target project folder, so the run is
/// inspectable/editable by hand even without the app's own data directory.
pub fn write_project_manifest(
    target_dir: &Path,
    template_id: &str,
    template_name: &str,
    template_description: &str,
    record: &RunRecord,
) -> Result<(), ProjectManifestError> {
    let dir = manifest_dir(target_dir);
    fs::create_dir_all(&dir)?;

    fs::write(dir.join("run.json"), serde_json::to_string_pretty(record)?)?;

    let snapshot = Template {
        id: template_id.to_string(),
        name: template_name.to_string(),
        description: template_description.to_string(),
        stages: record.resolved_stages.clone(),
    };
    fs::write(dir.join("pipeline.json"), serde_json::to_string_pretty(&snapshot)?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    fn sample_record() -> RunRecord {
        let stages = vec![Stage {
            id: "s1".to_string(),
            name: "Stage 1".to_string(),
            prompt: "do it".to_string(),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec![],
            checkpoint: true,
        }];
        RunRecord::new("run1".to_string(), "tpl1".to_string(), "/tmp/x".to_string(), &stages)
    }

    #[test]
    fn writes_run_json_and_pipeline_json_under_hidden_dir() {
        let target_dir = tempfile::tempdir().unwrap();
        let record = sample_record();

        write_project_manifest(target_dir.path(), "tpl1", "Template", "desc", &record).unwrap();

        let dir = manifest_dir(target_dir.path());
        let run_json: RunRecord =
            serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(run_json, record);

        let pipeline_json: Template =
            serde_json::from_str(&fs::read_to_string(dir.join("pipeline.json")).unwrap()).unwrap();
        assert_eq!(pipeline_json.id, "tpl1");
        assert_eq!(pipeline_json.name, "Template");
        assert_eq!(pipeline_json.stages, record.resolved_stages);
    }

    #[test]
    fn overwrites_existing_manifest_on_repeated_writes() {
        use crate::engine::run_record::RunStatus;

        let target_dir = tempfile::tempdir().unwrap();
        let mut record = sample_record();
        write_project_manifest(target_dir.path(), "tpl1", "Template", "desc", &record).unwrap();

        record.approve_current();
        write_project_manifest(target_dir.path(), "tpl1", "Template", "desc", &record).unwrap();

        let dir = manifest_dir(target_dir.path());
        let run_json: RunRecord =
            serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(run_json.status, RunStatus::Completed);
    }
}
