use std::fs;
use std::path::PathBuf;

use super::{is_valid_id, validate_template, Template, TemplateValidationError};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("validation error: {0}")]
    Validation(#[from] TemplateValidationError),
    #[error("template '{0}' not found")]
    NotFound(String),
    #[error("invalid id '{0}': ids must be non-empty, contain only letters, digits, '.', '_', or '-', and not be '.' or '..'")]
    InvalidId(String),
}

pub struct TemplateStore {
    dir: PathBuf,
}

impl TemplateStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    pub fn list(&self) -> Result<Vec<Template>, StoreError> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut templates = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                templates.push(serde_json::from_str(&content)?);
            }
        }
        templates.sort_by(|a: &Template, b: &Template| a.name.cmp(&b.name));
        Ok(templates)
    }

    pub fn load(&self, id: &str) -> Result<Template, StoreError> {
        if !is_valid_id(id) {
            return Err(StoreError::InvalidId(id.to_string()));
        }
        let path = self.path_for(id);
        if !path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, template: &Template) -> Result<(), StoreError> {
        validate_template(template)?;
        fs::create_dir_all(&self.dir)?;
        let content = serde_json::to_string_pretty(template)?;
        fs::write(self.path_for(&template.id), content)?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        if !is_valid_id(id) {
            return Err(StoreError::InvalidId(id.to_string()));
        }
        let path = self.path_for(id);
        if !path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    fn sample(id: &str) -> Template {
        Template {
            id: id.to_string(),
            name: "Sample".to_string(),
            description: "desc".to_string(),
            stages: vec![Stage {
                id: "s1".to_string(),
                name: "Stage 1".to_string(),
                prompt: "do it".to_string(),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec!["Read".to_string()],
                checkpoint: true,
            }],
        }
    }

    #[test]
    fn list_returns_empty_when_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path().join("does-not-exist"));
        assert_eq!(store.list().unwrap(), Vec::new());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        let template = sample("t1");
        store.save(&template).unwrap();
        assert_eq!(store.load("t1").unwrap(), template);
    }

    #[test]
    fn save_rejects_invalid_template() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        let mut template = sample("t1");
        template.stages.clear();
        assert!(matches!(store.save(&template), Err(StoreError::Validation(_))));
    }

    #[test]
    fn load_missing_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.load("missing"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn list_returns_saved_templates() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        store.save(&sample("a")).unwrap();
        store.save(&sample("b")).unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn delete_removes_template() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        store.save(&sample("t1")).unwrap();
        store.delete("t1").unwrap();
        assert!(matches!(store.load("t1"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn delete_missing_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.delete("missing"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn load_rejects_id_with_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.load("../../../etc/passwd"), Err(StoreError::InvalidId(_))));
    }

    #[test]
    fn load_rejects_id_with_slash() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.load("sub/dir"), Err(StoreError::InvalidId(_))));
    }

    #[test]
    fn delete_rejects_id_with_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.delete("../../../etc/passwd"), Err(StoreError::InvalidId(_))));
    }

    #[test]
    fn delete_rejects_id_with_slash() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        assert!(matches!(store.delete("sub/dir"), Err(StoreError::InvalidId(_))));
    }
}
