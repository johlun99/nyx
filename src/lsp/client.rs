// src/lsp/client.rs
//! LSP client: spawns a language server process and communicates via JSON-RPC.

use crate::lsp::protocol::{
    CodeAction, CompletionItem, CompletionResponse, DefinitionResponse, Diagnostic, Hover,
    Location, Position, Range, ServerCapabilities, TextDocumentIdentifier, TextDocumentItem,
    VersionedTextDocumentIdentifier, WorkspaceEdit,
};
use crate::lsp::transport::{read_message, write_message};
use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// Requests sent from the UI thread to the background thread.
#[derive(Debug)]
#[allow(dead_code)]
pub enum LspRequest {
    DidOpen {
        uri: String,
        language_id: String,
        version: i32,
        text: String,
    },
    DidChange {
        uri: String,
        version: i32,
        text: String,
    },
    DidClose {
        uri: String,
    },
    DidSave {
        uri: String,
        text: String,
    },
    Completion {
        uri: String,
        position: Position,
    },
    GotoDefinition {
        uri: String,
        position: Position,
    },
    References {
        uri: String,
        position: Position,
    },
    Hover {
        uri: String,
        position: Position,
    },
    Rename {
        uri: String,
        position: Position,
        new_name: String,
    },
    CodeAction {
        uri: String,
        range: Range,
    },
    Shutdown,
}

/// Responses sent from the background thread to the UI thread.
#[derive(Debug)]
pub enum LspResponse {
    Initialized(ServerCapabilities),
    Diagnostics {
        uri: String,
        diagnostics: Vec<Diagnostic>,
    },
    Completions(Vec<CompletionItem>),
    Definition(Vec<Location>),
    References(Vec<Location>),
    HoverResult(Option<Hover>),
    RenameResult(Option<WorkspaceEdit>),
    CodeActions(Vec<CodeAction>),
    ServerError(String),
    ServerStopped,
}

/// Handle for communicating with a running LSP client.
#[allow(dead_code)]
pub struct LspClientHandle {
    pub request_tx: Sender<LspRequest>,
    pub response_rx: Receiver<LspResponse>,
    pub server_name: String,
    pub capabilities: Option<ServerCapabilities>,
    _thread: std::thread::JoinHandle<()>,
}

/// Spawn an LSP client. Returns a handle for bidirectional communication.
pub fn spawn_client(
    server_name: &str,
    command: &str,
    args: &[&str],
    root_uri: &str,
) -> Result<LspClientHandle, String> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", command, e))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to capture stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture stderr".to_string())?;

    let (request_tx, request_rx) = crossbeam_channel::unbounded::<LspRequest>();
    let (response_tx, response_rx) = crossbeam_channel::unbounded::<LspResponse>();

    let name = server_name.to_string();
    let root = root_uri.to_string();

    let thread = std::thread::Builder::new()
        .name(format!("lsp-{}", server_name))
        .spawn(move || {
            client_thread(child, stdin, stdout, stderr, request_rx, response_tx, &root);
        })
        .map_err(|e| format!("Failed to spawn thread: {}", e))?;

    Ok(LspClientHandle {
        request_tx,
        response_rx,
        server_name: name,
        capabilities: None,
        _thread: thread,
    })
}

fn read_stderr(stderr: &mut std::process::ChildStderr) -> String {
    use std::io::Read;
    let mut buf = [0u8; 1024];
    match stderr.read(&mut buf) {
        Ok(n) if n > 0 => String::from_utf8_lossy(&buf[..n]).trim().to_string(),
        _ => String::new(),
    }
}

fn client_thread(
    mut child: Child,
    mut stdin: std::process::ChildStdin,
    stdout: std::process::ChildStdout,
    mut stderr: std::process::ChildStderr,
    request_rx: Receiver<LspRequest>,
    response_tx: Sender<LspResponse>,
    root_uri: &str,
) {
    let mut reader = BufReader::new(stdout);
    let next_id = Arc::new(AtomicI32::new(1));

    // Track pending request IDs to their method names for response dispatch
    let pending_requests: Arc<Mutex<HashMap<i64, String>>> = Arc::new(Mutex::new(HashMap::new()));

    // Send initialize request
    let init_id = next_id.fetch_add(1, Ordering::SeqCst);
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": init_id,
        "method": "initialize",
        "params": {
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "completion": {
                        "completionItem": {
                            "snippetSupport": false
                        }
                    },
                    "publishDiagnostics": {
                        "relatedInformation": false
                    },
                    "synchronization": {
                        "didSave": true
                    },
                    "hover": {
                        "contentFormat": ["plaintext"]
                    },
                    "definition": {},
                    "references": {},
                    "rename": {},
                    "codeAction": {}
                }
            }
        }
    });

    if write_message(&mut stdin, &init_request).is_err() {
        let stderr_msg = read_stderr(&mut stderr);
        let msg = if stderr_msg.is_empty() {
            "Failed to send initialize".to_string()
        } else {
            format!("Server error: {}", stderr_msg)
        };
        let _ = response_tx.send(LspResponse::ServerError(msg));
        let _ = child.kill();
        return;
    }

    // Read initialize response (skip any notifications the server sends first)
    let mut initialized = false;
    for _ in 0..20 {
        match read_message(&mut reader) {
            Ok(resp) => {
                // Check if this is the initialize response (has our request id)
                if resp.get("id").and_then(|v| v.as_i64()) == Some(init_id as i64) {
                    if let Some(result) = resp.get("result") {
                        if let Some(caps) = result.get("capabilities") {
                            let capabilities = ServerCapabilities::from_value(caps);
                            let _ = response_tx.send(LspResponse::Initialized(capabilities));
                            initialized = true;
                            break;
                        }
                    }
                    if let Some(error) = resp.get("error") {
                        let msg = error
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Initialize error");
                        let _ = response_tx.send(LspResponse::ServerError(msg.to_string()));
                        let _ = child.kill();
                        return;
                    }
                }
                // Otherwise it's a notification — skip and read next message
            }
            Err(e) => {
                let stderr_msg = read_stderr(&mut stderr);
                let msg = if stderr_msg.is_empty() {
                    format!("Failed to read initialize response: {}", e)
                } else {
                    format!("Server startup failed: {}", stderr_msg)
                };
                let _ = response_tx.send(LspResponse::ServerError(msg));
                let _ = child.kill();
                return;
            }
        }
    }
    if !initialized {
        let _ = response_tx.send(LspResponse::ServerError(
            "Initialize handshake timed out".to_string(),
        ));
        let _ = child.kill();
        return;
    }

    // Send initialized notification
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    let _ = write_message(&mut stdin, &initialized);

    // Main loop: read from both channels
    // We use a separate reader thread for stdout since read_message blocks.
    let (stdout_tx, stdout_rx) = crossbeam_channel::unbounded::<Value>();
    let reader_thread = std::thread::Builder::new()
        .name("lsp-reader".into())
        .spawn(move || {
            while let Ok(msg) = read_message(&mut reader) {
                if stdout_tx.send(msg).is_err() {
                    break;
                }
            }
        })
        .expect("spawn reader thread");

    loop {
        crossbeam_channel::select! {
            recv(request_rx) -> msg => {
                match msg {
                    Ok(LspRequest::DidOpen { uri, language_id, version, text }) => {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didOpen",
                            "params": {
                                "textDocument": TextDocumentItem {
                                    uri,
                                    language_id,
                                    version,
                                    text,
                                }
                            }
                        });
                        let _ = write_message(&mut stdin, &notification);
                    }
                    Ok(LspRequest::DidChange { uri, version, text }) => {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didChange",
                            "params": {
                                "textDocument": VersionedTextDocumentIdentifier {
                                    uri,
                                    version,
                                },
                                "contentChanges": [{ "text": text }]
                            }
                        });
                        let _ = write_message(&mut stdin, &notification);
                    }
                    Ok(LspRequest::DidClose { uri }) => {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didClose",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri }
                            }
                        });
                        let _ = write_message(&mut stdin, &notification);
                    }
                    Ok(LspRequest::DidSave { uri, text }) => {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didSave",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri },
                                "text": text
                            }
                        });
                        let _ = write_message(&mut stdin, &notification);
                    }
                    Ok(LspRequest::Completion { uri, position }) => {
                        let id = next_id.fetch_add(1, Ordering::SeqCst);
                        pending_requests.lock().unwrap().insert(id as i64, "textDocument/completion".to_string());
                        let request = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/completion",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri },
                                "position": position
                            }
                        });
                        let _ = write_message(&mut stdin, &request);
                    }
                    Ok(LspRequest::GotoDefinition { uri, position }) => {
                        let id = next_id.fetch_add(1, Ordering::SeqCst);
                        pending_requests.lock().unwrap().insert(id as i64, "textDocument/definition".to_string());
                        let request = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/definition",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri },
                                "position": position
                            }
                        });
                        let _ = write_message(&mut stdin, &request);
                    }
                    Ok(LspRequest::References { uri, position }) => {
                        let id = next_id.fetch_add(1, Ordering::SeqCst);
                        pending_requests.lock().unwrap().insert(id as i64, "textDocument/references".to_string());
                        let request = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/references",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri },
                                "position": position,
                                "context": { "includeDeclaration": true }
                            }
                        });
                        let _ = write_message(&mut stdin, &request);
                    }
                    Ok(LspRequest::Hover { uri, position }) => {
                        let id = next_id.fetch_add(1, Ordering::SeqCst);
                        pending_requests.lock().unwrap().insert(id as i64, "textDocument/hover".to_string());
                        let request = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/hover",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri },
                                "position": position
                            }
                        });
                        let _ = write_message(&mut stdin, &request);
                    }
                    Ok(LspRequest::Rename { uri, position, new_name }) => {
                        let id = next_id.fetch_add(1, Ordering::SeqCst);
                        pending_requests.lock().unwrap().insert(id as i64, "textDocument/rename".to_string());
                        let request = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/rename",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri },
                                "position": position,
                                "newName": new_name
                            }
                        });
                        let _ = write_message(&mut stdin, &request);
                    }
                    Ok(LspRequest::CodeAction { uri, range }) => {
                        let id = next_id.fetch_add(1, Ordering::SeqCst);
                        pending_requests.lock().unwrap().insert(id as i64, "textDocument/codeAction".to_string());
                        let request = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "textDocument/codeAction",
                            "params": {
                                "textDocument": TextDocumentIdentifier { uri },
                                "range": range,
                                "context": { "diagnostics": [] }
                            }
                        });
                        let _ = write_message(&mut stdin, &request);
                    }
                    Ok(LspRequest::Shutdown) => {
                        let id = next_id.fetch_add(1, Ordering::SeqCst);
                        let shutdown = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "shutdown",
                            "params": null
                        });
                        let _ = write_message(&mut stdin, &shutdown);

                        let exit = json!({
                            "jsonrpc": "2.0",
                            "method": "exit",
                            "params": null
                        });
                        let _ = write_message(&mut stdin, &exit);
                        let _ = response_tx.send(LspResponse::ServerStopped);
                        break;
                    }
                    Err(_) => break,
                }
            }
            recv(stdout_rx) -> msg => {
                match msg {
                    Ok(value) => {
                        handle_server_message(&value, &response_tx, &pending_requests);
                    }
                    Err(_) => {
                        let _ = response_tx.send(LspResponse::ServerStopped);
                        break;
                    }
                }
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = reader_thread.join();
}

fn handle_server_message(
    msg: &Value,
    response_tx: &Sender<LspResponse>,
    pending_requests: &Arc<Mutex<HashMap<i64, String>>>,
) {
    // Notification: textDocument/publishDiagnostics
    if msg.get("method").and_then(|m| m.as_str()) == Some("textDocument/publishDiagnostics") {
        if let Some(params) = msg.get("params") {
            let uri = params
                .get("uri")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            let diagnostics: Vec<Diagnostic> = params
                .get("diagnostics")
                .and_then(|d| serde_json::from_value(d.clone()).ok())
                .unwrap_or_default();
            let _ = response_tx.send(LspResponse::Diagnostics { uri, diagnostics });
        }
        return;
    }

    // Response with id — look up method from pending requests
    if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
        let method = pending_requests.lock().unwrap().remove(&id);

        if let Some(error) = msg.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            let _ = response_tx.send(LspResponse::ServerError(message));
            return;
        }

        let result = msg.get("result");

        match method.as_deref() {
            Some("textDocument/completion") => {
                if let Some(result) = result {
                    if let Ok(completion) =
                        serde_json::from_value::<CompletionResponse>(result.clone())
                    {
                        let _ = response_tx.send(LspResponse::Completions(completion.into_items()));
                    }
                }
            }
            Some("textDocument/definition") => {
                let locs = result
                    .and_then(|r| {
                        if r.is_null() {
                            return Some(Vec::new());
                        }
                        serde_json::from_value::<DefinitionResponse>(r.clone())
                            .ok()
                            .map(|d| d.into_locations())
                    })
                    .unwrap_or_default();
                let _ = response_tx.send(LspResponse::Definition(locs));
            }
            Some("textDocument/references") => {
                let locs = result
                    .and_then(|r| {
                        if r.is_null() {
                            return Some(Vec::new());
                        }
                        serde_json::from_value::<Vec<Location>>(r.clone()).ok()
                    })
                    .unwrap_or_default();
                let _ = response_tx.send(LspResponse::References(locs));
            }
            Some("textDocument/hover") => {
                let hover = result.and_then(|r| {
                    if r.is_null() {
                        return None;
                    }
                    serde_json::from_value::<Hover>(r.clone()).ok()
                });
                let _ = response_tx.send(LspResponse::HoverResult(hover));
            }
            Some("textDocument/rename") => {
                let edit = result.and_then(|r| {
                    if r.is_null() {
                        return None;
                    }
                    serde_json::from_value::<WorkspaceEdit>(r.clone()).ok()
                });
                let _ = response_tx.send(LspResponse::RenameResult(edit));
            }
            Some("textDocument/codeAction") => {
                let actions = result
                    .and_then(|r| {
                        if r.is_null() {
                            return Some(Vec::new());
                        }
                        serde_json::from_value::<Vec<CodeAction>>(r.clone()).ok()
                    })
                    .unwrap_or_default();
                let _ = response_tx.send(LspResponse::CodeActions(actions));
            }
            _ => {
                // Unknown or untracked response — ignore
            }
        }
    }

    // Other notifications we don't handle — ignore
}
