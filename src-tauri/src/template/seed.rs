use super::store::{StoreError, TemplateStore};
use super::{PermissionMode, Stage, Template};

fn stage(id: &str, name: &str, prompt: &str, allowed_tools: &[&str]) -> Stage {
    Stage {
        id: id.to_string(),
        name: name.to_string(),
        prompt: prompt.to_string(),
        permission_mode: PermissionMode::AcceptEdits,
        allowed_tools: allowed_tools.iter().map(|s| s.to_string()).collect(),
        checkpoint: true,
    }
}

fn default_templates() -> Vec<Template> {
    vec![
        Template {
            id: "web-app-dev".to_string(),
            name: "웹 프로그램 개발".to_string(),
            description: "요구사항 정의부터 배포까지 웹 애플리케이션 개발 전체 파이프라인".to_string(),
            stages: vec![
                stage(
                    "requirements",
                    "요구사항 정의",
                    "PRD.md를 작성하라. project-architect 스킬을 사용해 프로젝트의 목적, 사용자, 핵심 기능, 성공 기준을 정리하라.",
                    &["Read", "Write", "Glob", "Grep", "WebSearch"],
                ),
                stage(
                    "design",
                    "아키텍처 설계",
                    "PRD.md를 참고해 시스템 아키텍처와 데이터 모델을 설계하고 ARCHITECTURE.md에 문서화하라. project-architect 스킬을 사용하라.",
                    &["Read", "Write", "Glob", "Grep"],
                ),
                stage(
                    "frontend",
                    "프론트엔드 구현",
                    "ARCHITECTURE.md를 참고해 프론트엔드 UI를 구현하라. frontend-craftsman 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Glob", "Grep", "Bash"],
                ),
                stage(
                    "backend",
                    "백엔드 구현",
                    "ARCHITECTURE.md를 참고해 백엔드 API를 구현하라. api-engineer 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Glob", "Grep", "Bash"],
                ),
                stage(
                    "testing",
                    "테스트",
                    "구현된 기능에 대한 테스트를 작성하고 실행하라. qa-tester 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Bash"],
                ),
                stage(
                    "deployment",
                    "배포 스크립트",
                    "빌드 및 배포 스크립트를 작성하라. devops-builder 스킬을 사용하라.",
                    &["Read", "Write", "Bash"],
                ),
            ],
        },
        Template {
            id: "desktop-app-dev-tauri".to_string(),
            name: "데스크톱 프로그램 개발 (Tauri)".to_string(),
            description: "요구사항 정의부터 패키징까지 Tauri 데스크톱 애플리케이션 개발 전체 파이프라인".to_string(),
            stages: vec![
                stage(
                    "requirements",
                    "요구사항 정의",
                    "PRD.md를 작성하라. project-architect 스킬을 사용해 프로젝트의 목적, 사용자, 핵심 기능, 성공 기준을 정리하라.",
                    &["Read", "Write", "Glob", "Grep", "WebSearch"],
                ),
                stage(
                    "design",
                    "아키텍처 설계",
                    "PRD.md를 참고해 Tauri(Rust 백엔드 + 프론트엔드) 아키텍처를 설계하고 ARCHITECTURE.md에 문서화하라. project-architect 스킬을 사용하라.",
                    &["Read", "Write", "Glob", "Grep"],
                ),
                stage(
                    "implementation",
                    "구현",
                    "ARCHITECTURE.md를 참고해 Tauri 프론트엔드와 Rust 백엔드를 구현하라. frontend-craftsman 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Glob", "Grep", "Bash"],
                ),
                stage(
                    "testing",
                    "테스트",
                    "구현된 기능에 대한 테스트를 작성하고 실행하라. qa-tester 스킬을 사용하라.",
                    &["Read", "Write", "Edit", "Bash"],
                ),
                stage(
                    "packaging",
                    "패키징",
                    "MSI/DMG/AppImage 인스톨 패키지를 생성하는 배포 스크립트를 작성하라. devops-builder 스킬을 사용하라.",
                    &["Read", "Write", "Bash"],
                ),
            ],
        },
    ]
}

pub fn seed_default_templates(store: &TemplateStore) -> Result<(), StoreError> {
    for template in default_templates() {
        if store.load(&template.id).is_err() {
            store.save(&template)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{PermissionMode, Stage};

    #[test]
    fn creates_default_templates_when_store_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        seed_default_templates(&store).unwrap();
        let templates = store.list().unwrap();
        assert_eq!(templates.len(), 2);
        assert!(templates.iter().any(|t| t.id == "web-app-dev"));
        assert!(templates.iter().any(|t| t.id == "desktop-app-dev-tauri"));
    }

    #[test]
    fn does_not_overwrite_a_customized_existing_template() {
        let dir = tempfile::tempdir().unwrap();
        let store = TemplateStore::new(dir.path());
        let customized = Template {
            id: "web-app-dev".to_string(),
            name: "내가 수정한 이름".to_string(),
            description: "desc".to_string(),
            stages: vec![Stage {
                id: "only-stage".to_string(),
                name: "Only".to_string(),
                prompt: "do it".to_string(),
                permission_mode: PermissionMode::AcceptEdits,
                allowed_tools: vec![],
                checkpoint: true,
            }],
        };
        store.save(&customized).unwrap();

        seed_default_templates(&store).unwrap();

        let loaded = store.load("web-app-dev").unwrap();
        assert_eq!(loaded.name, "내가 수정한 이름");
    }

    #[test]
    fn seeded_templates_pass_validation() {
        for template in default_templates() {
            crate::template::validate_template(&template).unwrap();
        }
    }
}
