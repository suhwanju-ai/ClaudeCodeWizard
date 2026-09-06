use serde::{Deserialize, Serialize};

pub mod store;

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
}

pub fn validate_template(template: &Template) -> Result<(), TemplateValidationError> {
    if template.stages.is_empty() {
        return Err(TemplateValidationError::NoStages);
    }
    let mut seen = std::collections::HashSet::new();
    for stage in &template.stages {
        if stage.prompt.trim().is_empty() {
            return Err(TemplateValidationError::EmptyPrompt(stage.id.clone()));
        }
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
    fn serializes_permission_mode_as_camel_case() {
        let json = serde_json::to_string(&PermissionMode::AcceptEdits).unwrap();
        assert_eq!(json, "\"acceptEdits\"");
    }
}
