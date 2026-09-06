use claude_pipeline_wizard_lib::cli_check::{check_claude_cli, CliCheckResult};

#[test]
fn returns_available_with_version_for_mock_binary() {
    let binary = env!("CARGO_BIN_EXE_mock_claude");
    assert_eq!(
        check_claude_cli(binary),
        CliCheckResult::Available("mock-claude 0.0.1".to_string())
    );
}
