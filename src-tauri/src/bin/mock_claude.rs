use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();

    if let Ok(dump_path) = env::var("MOCK_CLAUDE_DUMP_ARGS") {
        let _ = fs::write(dump_path, args.join("\n"));
    }

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
        let mut lines = content.lines().peekable();
        if let Some(first_line) = lines.peek() {
            if let Some(ms) = first_line.strip_prefix("SLEEP:") {
                if let Ok(ms) = ms.trim().parse::<u64>() {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
                lines.next();
            }
        }
        for line in lines {
            println!("{line}");
        }
        let stderr_path = format!("{fixture_path}.stderr");
        if let Ok(stderr_text) = fs::read_to_string(&stderr_path) {
            eprint!("{stderr_text}");
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
