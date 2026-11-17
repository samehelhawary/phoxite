//! A language server on stdio, for the tests that ask another server
//! (Intelephense) the same things as the engine: `memory.rs` and
//! `differential.rs`.
//!
//! The server's own requests are answered while a request of ours waits:
//! `workspace/configuration` with defaults, anything else with `null`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    messages: mpsc::Receiver<Value>,
    next_id: i64,
}

impl LspClient {
    /// Starts `command`, which speaks LSP on its stdin and stdout.
    pub fn spawn(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("can't start {:?}: {e}", command.get_program()));
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, messages) = mpsc::channel::<Value>();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if tx.send(message).is_err() {
                    return;
                }
            }
        });
        Self { child, stdin, messages, next_id: 0 }
    }

    /// The server's process id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Sends a request and waits for its result; an error is the server's
    /// message, or that it didn't answer within `timeout`.
    pub fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        self.next_id += 1;
        let id = json!(self.next_id);
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let deadline = Instant::now() + timeout;
        loop {
            let message = self.next(deadline).ok_or_else(|| "timed out".to_owned())?;
            if message.get("method").is_none() && message.get("id") == Some(&id) {
                return match message.get("error") {
                    Some(error) => Err(error["message"].as_str().unwrap_or("error").to_owned()),
                    None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                };
            }
        }
    }

    /// Waits for the server's notification `method`; whether it came within
    /// `timeout`.
    pub fn wait_for(&mut self, method: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while let Some(message) = self.next(deadline) {
            if message.get("id").is_none() && message.get("method").and_then(Value::as_str) == Some(method) {
                return true;
            }
        }
        false
    }

    /// The next message that isn't a request of the server (those are
    /// answered here); `None` at `deadline` or when the server is gone.
    fn next(&mut self, deadline: Instant) -> Option<Value> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = self.messages.recv_timeout(left).ok()?;
            let method = message.get("method").and_then(Value::as_str);
            match (message.get("id"), method) {
                (Some(id), Some(method)) => {
                    let result = if method == "workspace/configuration" {
                        let items = message["params"]["items"].as_array().map_or(0, Vec::len);
                        Value::Array(vec![Value::Null; items])
                    } else {
                        Value::Null
                    };
                    let answer = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                    self.send(&answer);
                }
                _ => return Some(message),
            }
        }
    }

    fn send(&mut self, message: &Value) {
        let body = message.to_string();
        // The server may already be gone after `exit`.
        let _ = write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len());
        let _ = self.stdin.flush();
    }

    /// Asks the server to leave, and kills it when it doesn't within
    /// `timeout`.
    pub fn shutdown(mut self, timeout: Duration) {
        let _ = self.request("shutdown", Value::Null, timeout);
        self.notify("exit", Value::Null);
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

/// A path as a `file:` URI.
pub fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') { format!("file://{text}") } else { format!("file:///{text}") }
}

/// The path of a `file:` URI, percent-decoded (`file:///d%3A/x` is
/// `d:/x`); another URI as it is.
pub fn uri_path(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("file://") else { return uri.to_owned() };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(high), Some(low)) => {
                out.push(high * 16 + low);
                i += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    let path = String::from_utf8_lossy(&out).into_owned();
    // `/d:/x` on Windows.
    match path.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => path[1..].to_owned(),
        _ => path,
    }
}

/// Intelephense as `npm i -g intelephense` installs it: its script, for
/// `node`, and its version.
pub fn intelephense_script() -> (PathBuf, String) {
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let global = Command::new(npm).args(["root", "-g"]).output().expect("npm (Node 18+) on the PATH");
    let package = PathBuf::from(String::from_utf8_lossy(&global.stdout).trim()).join("intelephense");
    let script = package.join("lib/intelephense.js");
    assert!(script.is_file(), "{} missing: npm i -g intelephense", script.display());
    let version = std::fs::read_to_string(package.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|package| package["version"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "?".into());
    (script, version)
}

/// Starts Intelephense (with `node` directly, so the process is the server
/// and not a shell) on `root` with an empty index in `storage`, and waits
/// until it has indexed the folder. Returns it with how long that took.
pub fn intelephense(root: &Path, storage: &Path, timeout: Duration) -> (LspClient, Duration) {
    let (script, _) = intelephense_script();
    let mut command = Command::new("node");
    command.arg(&script).arg("--stdio");
    let mut client = LspClient::spawn(command);
    let uri = file_uri(root);
    let started = Instant::now();
    let params = json!({
        "processId": std::process::id(),
        "rootUri": uri,
        "workspaceFolders": [{ "uri": uri, "name": "project" }],
        "capabilities": {
            "workspace": { "configuration": true, "workspaceFolders": true },
            "textDocument": {
                "hover": { "contentFormat": ["markdown", "plaintext"] },
                // With links a definition is the declaration's name; without, Intelephense
                // answers with the range of the whole declaration, its docblock included.
                "definition": { "linkSupport": true },
            },
        },
        "initializationOptions": { "storagePath": storage.to_string_lossy(), "clearCache": true },
    });
    // Intelephense 1.18 crashes when `initialized` comes before the
    // `initialize` result.
    client.request("initialize", params, timeout).expect("Intelephense starts");
    client.notify("initialized", json!({}));
    assert!(client.wait_for("indexingEnded", timeout), "Intelephense didn't finish indexing in {timeout:?}");
    (client, started.elapsed())
}
