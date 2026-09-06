use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StageEvent {
    Init {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    AssistantText { text: String },
    ToolUse { name: String, input: serde_json::Value },
    ToolResult { content: serde_json::Value },
    Result {
        #[serde(rename = "sessionId")]
        session_id: String,
        success: bool,
        result: Option<String>,
    },
    /// Synthesized by the executor (never parsed from a stream-json line) when the
    /// claude process exits non-zero, times out, or its stdout can't be read — carries
    /// whatever it wrote to stderr so the failure is diagnosable from the run log.
    ProcessError {
        #[serde(rename = "exitCode")]
        exit_code: Option<i32>,
        stderr: String,
    },
    Unknown { raw: serde_json::Value },
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("invalid stream-json line: {0}")]
pub struct ParseWarning(pub String);

pub fn parse_line(line: &str) -> Result<StageEvent, ParseWarning> {
    let trimmed = line.trim();
    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| ParseWarning(format!("{e}: {trimmed}")))?;
    let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match event_type {
        "system" => {
            let session_id = value.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(StageEvent::Init { session_id })
        }
        "assistant" => {
            let text = value
                .pointer("/message/content")
                .and_then(|c| c.as_array())
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            Ok(StageEvent::AssistantText { text })
        }
        "tool_use" => {
            let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let input = value.get("input").cloned().unwrap_or(serde_json::Value::Null);
            Ok(StageEvent::ToolUse { name, input })
        }
        "tool_result" => {
            let content = value.get("content").cloned().unwrap_or(serde_json::Value::Null);
            Ok(StageEvent::ToolResult { content })
        }
        "result" => {
            let session_id = value.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let subtype = value.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
            let result = value.get("result").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(StageEvent::Result { session_id, success: subtype == "success", result })
        }
        _ => Ok(StageEvent::Unknown { raw: value }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_init_event() {
        let line = r#"{"type":"system","session_id":"sess-1"}"#;
        assert_eq!(parse_line(line).unwrap(), StageEvent::Init { session_id: "sess-1".to_string() });
    }

    #[test]
    fn parses_assistant_text_event() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        assert_eq!(parse_line(line).unwrap(), StageEvent::AssistantText { text: "hello".to_string() });
    }

    #[test]
    fn parses_tool_use_event() {
        let line = r#"{"type":"tool_use","name":"Write","input":{"path":"a.txt"}}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::ToolUse { name: "Write".to_string(), input: serde_json::json!({"path": "a.txt"}) }
        );
    }

    #[test]
    fn parses_tool_result_event() {
        let line = r#"{"type":"tool_result","content":"ok"}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::ToolResult { content: serde_json::json!("ok") }
        );
    }

    #[test]
    fn parses_successful_result_event() {
        let line = r#"{"type":"result","subtype":"success","session_id":"sess-1","result":"done"}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::Result { session_id: "sess-1".to_string(), success: true, result: Some("done".to_string()) }
        );
    }

    #[test]
    fn parses_failed_result_event() {
        let line = r#"{"type":"result","subtype":"error","session_id":"sess-1"}"#;
        assert_eq!(
            parse_line(line).unwrap(),
            StageEvent::Result { session_id: "sess-1".to_string(), success: false, result: None }
        );
    }

    #[test]
    fn unknown_type_becomes_unknown_event() {
        let line = r#"{"type":"future_event","foo":"bar"}"#;
        let event = parse_line(line).unwrap();
        assert!(matches!(event, StageEvent::Unknown { .. }));
    }

    #[test]
    fn invalid_json_returns_parse_warning() {
        assert!(parse_line("not json").is_err());
    }

    #[test]
    fn serializes_with_camel_case_kind_tag() {
        let event = StageEvent::Init { session_id: "sess-1".to_string() };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"kind":"init","sessionId":"sess-1"}"#);
    }

    #[test]
    fn serializes_process_error_with_camel_case_exit_code() {
        let event = StageEvent::ProcessError { exit_code: Some(1), stderr: "boom".to_string() };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"kind":"processError","exitCode":1,"stderr":"boom"}"#);
    }
}
