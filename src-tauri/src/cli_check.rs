use std::process::Command;

#[derive(Debug, PartialEq)]
pub enum CliCheckResult {
    Available(String),
    NotFound,
}

pub fn check_claude_cli(binary: &str) -> CliCheckResult {
    match Command::new(binary).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            CliCheckResult::Available(version)
        }
        _ => CliCheckResult::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_not_found_for_missing_binary() {
        assert_eq!(check_claude_cli("definitely-not-a-real-binary-xyz"), CliCheckResult::NotFound);
    }
}
