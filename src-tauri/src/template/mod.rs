use serde::{Deserialize, Serialize};

pub mod store;
pub mod seed;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    AcceptEdits,
    BypassPermissions,
    Default,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub permission_mode: PermissionMode,
    pub allowed_tools: Vec<String>,
    pub checkpoint: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stages: Vec<Stage>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum TemplateValidationError {
    #[error("template must have at least one stage")]
    NoStages,
    #[error("stage '{0}' has an empty prompt")]
    EmptyPrompt(String),
    #[error("duplicate stage id '{0}'")]
    DuplicateStageId(String),
    #[error("invalid id '{0}': ids must be non-empty, contain only letters, digits, '.', '_', or '-', and not be '.' or '..'")]
    InvalidId(String),
}

pub(crate) fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Validates a single stage in isolation: id charset first, then a non-blank prompt.
/// Template-level rules (at least one stage, unique stage ids) stay in `validate_template`.
pub fn validate_stage(stage: &Stage) -> Result<(), TemplateValidationError> {
    if !is_valid_id(&stage.id) {
        return Err(TemplateValidationError::InvalidId(stage.id.clone()));
    }
    if stage.prompt.trim().is_empty() {
        return Err(TemplateValidationError::EmptyPrompt(stage.id.clone()));
    }
    Ok(())
}

pub fn validate_template(template: &Template) -> Result<(), TemplateValidationError> {
    if !is_valid_id(&template.id) {
        return Err(TemplateValidationError::InvalidId(template.id.clone()));
    }
    if template.stages.is_empty() {
        return Err(TemplateValidationError::NoStages);
    }
    let mut seen = std::collections::HashSet::new();
    for stage in &template.stages {
        validate_stage(stage)?;
        if !seen.insert(stage.id.clone()) {
            return Err(TemplateValidationError::DuplicateStageId(stage.id.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(id: &str, prompt: &str) -> Stage {
        Stage {
            id: id.to_string(),
            name: id.to_string(),
            prompt: prompt.to_string(),
            permission_mode: PermissionMode::AcceptEdits,
            allowed_tools: vec!["Read".to_string()],
            checkpoint: true,
        }
    }

    fn template(stages: Vec<Stage>) -> Template {
        Template {
            id: "t1".to_string(),
            name: "Test".to_string(),
            description: "desc".to_string(),
            stages,
        }
    }

    #[test]
    fn rejects_empty_stage_list() {
        assert_eq!(validate_template(&template(vec![])), Err(TemplateValidationError::NoStages));
    }

    #[test]
    fn rejects_empty_prompt() {
        let t = template(vec![stage("s1", "   ")]);
        assert_eq!(validate_template(&t), Err(TemplateValidationError::EmptyPrompt("s1".to_string())));
    }

    #[test]
    fn rejects_duplicate_stage_ids() {
        let t = template(vec![stage("s1", "do it"), stage("s1", "do it again")]);
        assert_eq!(validate_template(&t), Err(TemplateValidationError::DuplicateStageId("s1".to_string())));
    }

    #[test]
    fn accepts_valid_template() {
        let t = template(vec![stage("s1", "do it"), stage("s2", "do more")]);
        assert_eq!(validate_template(&t), Ok(()));
    }

    #[test]
    fn rejects_template_id_with_path_separator() {
        let mut t = template(vec![stage("s1", "do it")]);
        t.id = "../../etc/passwd".to_string();
        assert_eq!(validate_template(&t), Err(TemplateValidationError::InvalidId(t.id.clone())));
    }

    #[test]
    fn rejects_template_id_of_dot_dot() {
        let mut t = template(vec![stage("s1", "do it")]);
        t.id = "..".to_string();
        assert_eq!(validate_template(&t), Err(TemplateValidationError::InvalidId("..".to_string())));
    }

    #[test]
    fn rejects_empty_template_id() {
        let mut t = template(vec![stage("s1", "do it")]);
        t.id = "".to_string();
        assert_eq!(validate_template(&t), Err(TemplateValidationError::InvalidId("".to_string())));
    }

    #[test]
    fn accepts_normal_template_id() {
        let mut t = template(vec![stage("s1", "do it")]);
        t.id = "web-app-dev".to_string();
        assert_eq!(validate_template(&t), Ok(()));
    }

    #[test]
    fn rejects_stage_id_with_path_separator() {
        let t = template(vec![stage("../bad", "do it")]);
        assert_eq!(validate_template(&t), Err(TemplateValidationError::InvalidId("../bad".to_string())));
    }

    #[test]
    fn serializes_permission_mode_as_camel_case() {
        let json = serde_json::to_string(&PermissionMode::AcceptEdits).unwrap();
        assert_eq!(json, "\"acceptEdits\"");
    }

    #[test]
    fn validate_stage_accepts_valid_stage() {
        assert_eq!(validate_stage(&stage("s1", "do it")), Ok(()));
    }

    #[test]
    fn validate_stage_rejects_empty_prompt() {
        assert_eq!(
            validate_stage(&stage("s1", "   ")),
            Err(TemplateValidationError::EmptyPrompt("s1".to_string()))
        );
    }

    #[test]
    fn validate_stage_rejects_invalid_id() {
        assert_eq!(
            validate_stage(&stage("../bad", "do it")),
            Err(TemplateValidationError::InvalidId("../bad".to_string()))
        );
    }

    #[test]
    fn validate_stage_checks_id_before_prompt() {
        // validate_template reports InvalidId before EmptyPrompt for the same stage;
        // the extracted helper must preserve that order (PRD F-1).
        assert_eq!(
            validate_stage(&stage("../bad", "")),
            Err(TemplateValidationError::InvalidId("../bad".to_string()))
        );
    }
}
