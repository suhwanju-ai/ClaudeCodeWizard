use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version") {
        println!("mock-claude 0.0.1");
        exit(0);
    }

    let prompt = args
        .iter()
        .position(|a| a == "-p")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();

    if let Some(fixture_path) = prompt.strip_prefix("FIXTURE:") {
        let content = fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("mock_claude: cannot read fixture {fixture_path}: {e}"));
        for line in content.lines() {
            println!("{line}");
        }
        let exit_code_path = format!("{fixture_path}.exitcode");
        let code: i32 = fs::read_to_string(&exit_code_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        exit(code);
    }

    exit(0);
}
