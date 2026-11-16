use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn stdio_mcp_reads_projects_and_keeps_stdout_protocol_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.json");
    let mut project = m4b_core::Project::default();
    project.metadata.title = "MCP book".into();
    project.save(&path).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_m4b-cli"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(input, "not-json").unwrap();
    for request in [
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"inspect_project","arguments":{"path":path}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"inspect_project","arguments":{"path":dir.path().join("missing.json")}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"unknown"}),
        json!({"id":6,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"inspect_project","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"unknown","arguments":{"path":path}}}),
    ] {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(responses.len(), 9);
    assert_eq!(responses[0]["error"]["code"], -32700);
    assert_eq!(responses[1]["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(responses[2]["result"]["tools"].as_array().unwrap().len(), 2);
    let loaded: Value = serde_json::from_str(
        responses[3]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(loaded["metadata"]["title"], "MCP book");
    assert_eq!(responses[4]["result"]["isError"], true);
    assert_eq!(responses[5]["error"]["code"], -32601);
    assert_eq!(responses[6]["error"]["code"], -32600);
    assert_eq!(responses[7]["error"]["code"], -32602);
    assert_eq!(responses[8]["error"]["code"], -32602);
    for response in responses {
        assert_eq!(response["jsonrpc"], "2.0");
    }
}
