use crossbeam_channel::{bounded, Receiver, Sender};
use eframe::egui;
use std::io::BufRead;
use std::process::{Command, Stdio};

use super::provider::{
    ChatContext, ProviderConfig, ProviderEvent, ProviderHandle, ProviderRequest,
};

/// Spawn a background thread that manages CLI subprocess communication.
/// Returns a ProviderHandle with channels for sending requests and receiving events.
pub fn spawn_provider(
    config: ProviderConfig,
    egui_ctx: egui::Context,
) -> Result<ProviderHandle, String> {
    let (request_tx, request_rx) = bounded::<ProviderRequest>(16);
    let (event_tx, event_rx) = bounded::<ProviderEvent>(256);

    let thread_config = config.clone();
    std::thread::Builder::new()
        .name("ai-provider".to_string())
        .spawn(move || {
            provider_thread(thread_config, request_rx, event_tx, egui_ctx);
        })
        .map_err(|e| format!("Failed to spawn provider thread: {}", e))?;

    Ok(ProviderHandle {
        request_tx,
        event_rx,
        config,
    })
}

fn provider_thread(
    config: ProviderConfig,
    request_rx: Receiver<ProviderRequest>,
    event_tx: Sender<ProviderEvent>,
    egui_ctx: egui::Context,
) {
    loop {
        match request_rx.recv() {
            Ok(ProviderRequest::SendMessage { prompt, context }) => {
                run_cli_subprocess(&config, &prompt, context, &request_rx, &event_tx, &egui_ctx);
            }
            Ok(ProviderRequest::Cancel) => {
                // Nothing running, ignore
            }
            Ok(ProviderRequest::Shutdown) | Err(_) => {
                break;
            }
        }
    }
}

fn build_full_prompt(prompt: &str, context: Option<ChatContext>) -> String {
    let mut parts = Vec::new();

    if let Some(ctx) = context {
        if let Some(ref wd) = ctx.working_directory {
            parts.push(format!("Working directory: {}", wd.display()));
        }
        if let Some(ref path) = ctx.file_path {
            parts.push(format!("Current file: {}", path));
        }
        if let Some(ref content) = ctx.file_content {
            // Only include file content if it's not too large
            if content.len() < 50_000 {
                parts.push(format!("File content:\n```\n{}\n```", content));
            }
        }
        if let Some(ref sel) = ctx.selection {
            parts.push(format!("Selected text:\n```\n{}\n```", sel));
        }
    }

    parts.push(prompt.to_string());
    parts.join("\n\n")
}

fn run_cli_subprocess(
    config: &ProviderConfig,
    prompt: &str,
    context: Option<ChatContext>,
    request_rx: &Receiver<ProviderRequest>,
    event_tx: &Sender<ProviderEvent>,
    egui_ctx: &egui::Context,
) {
    let full_prompt = build_full_prompt(prompt, context);

    // Build command: e.g. claude -p "prompt" --output-format stream-json
    let mut cmd = Command::new(&config.command);

    // Add configured args, inserting the prompt after "-p"
    for arg in &config.args {
        cmd.arg(arg);
        if arg == "-p" {
            cmd.arg(&full_prompt);
        }
    }

    // Set configured env vars
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    // Must unset Claude env vars to avoid "nested session" error
    cmd.env_remove("CLAUDECODE");
    cmd.env_remove("CLAUDE_CODE_ENTRYPOINT");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = event_tx.send(ProviderEvent::Error(format!(
                "Failed to start '{}': {}",
                config.command, e
            )));
            egui_ctx.request_repaint();
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = event_tx.send(ProviderEvent::Error("Failed to capture stdout".to_string()));
            egui_ctx.request_repaint();
            return;
        }
    };

    let reader = std::io::BufReader::new(stdout);
    let mut token_usage = None;
    let mut context_window = None;

    for line in reader.lines() {
        // Check for cancellation
        if let Ok(req) = request_rx.try_recv() {
            match req {
                ProviderRequest::Cancel => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = event_tx.send(ProviderEvent::Done {
                        token_usage: None,
                        context_window: None,
                    });
                    egui_ctx.request_repaint();
                    return;
                }
                ProviderRequest::Shutdown => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                _ => {}
            }
        }

        let line = match line {
            Ok(line) => line,
            Err(_) => continue,
        };

        if line.trim().is_empty() {
            continue;
        }

        // Parse stream-json line
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            // Check for status/progress messages
            if let Some(status) = extract_status(&json) {
                let _ = event_tx.send(ProviderEvent::Status(status));
                egui_ctx.request_repaint();
            }

            if let Some(text) = extract_text_token(&json) {
                if event_tx.send(ProviderEvent::Token(text)).is_err() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                egui_ctx.request_repaint();
            }

            // Check for usage data in result messages
            if let Some(usage) = extract_token_usage(&json) {
                token_usage = Some(usage);
            }

            // Extract context window from result's modelUsage
            if context_window.is_none() {
                if let Some(cw) = extract_context_window(&json) {
                    context_window = Some(cw);
                }
            }
        }
    }

    // Wait for process to finish
    let stderr_output = child
        .stderr
        .take()
        .and_then(|stderr| std::io::read_to_string(stderr).ok())
        .unwrap_or_default();

    let status = child.wait();

    if let Ok(status) = status {
        if !status.success() {
            let err_msg = if stderr_output.trim().is_empty() {
                format!("Process exited with status: {}", status)
            } else {
                stderr_output.trim().to_string()
            };
            let _ = event_tx.send(ProviderEvent::Error(err_msg));
            egui_ctx.request_repaint();
            return;
        }
    }

    let _ = event_tx.send(ProviderEvent::Done {
        token_usage,
        context_window,
    });
    egui_ctx.request_repaint();
}

/// Extract text content from a stream-json line.
/// Claude CLI stream-json format: {"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}
fn extract_text_token(json: &serde_json::Value) -> Option<String> {
    let msg_type = json.get("type").and_then(|t| t.as_str())?;

    match msg_type {
        // Main response: {"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}
        "assistant" => {
            let content = json.get("message")?.get("content")?.as_array()?;
            let mut text = String::new();
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        text.push_str(t);
                    }
                }
            }
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        // Content block delta (streaming chunks)
        "content_block_delta" => json
            .get("delta")?
            .get("text")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// Extract token usage from a result message.
fn extract_token_usage(json: &serde_json::Value) -> Option<usize> {
    // Check for result type with usage info
    if json.get("type").and_then(|t| t.as_str()) == Some("result") {
        if let Some(usage) = json.get("usage") {
            let input = usage
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let output = usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            return Some((input + output) as usize);
        }
    }

    // Check for message_stop with usage
    if json.get("type").and_then(|t| t.as_str()) == Some("message_stop") {
        if let Some(usage) = json.get("usage") {
            let input = usage
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let output = usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            return Some((input + output) as usize);
        }
    }

    None
}

/// Extract context window size from result's modelUsage.
/// Format: {"type":"result","modelUsage":{"claude-opus-4-6":{"contextWindow":200000,...}}}
fn extract_context_window(json: &serde_json::Value) -> Option<usize> {
    if json.get("type").and_then(|t| t.as_str()) != Some("result") {
        return None;
    }
    let model_usage = json.get("modelUsage")?.as_object()?;
    for (_model, usage) in model_usage {
        if let Some(cw) = usage.get("contextWindow").and_then(|v| v.as_u64()) {
            return Some(cw as usize);
        }
    }
    None
}

/// Extract progress status from system messages.
fn extract_status(json: &serde_json::Value) -> Option<String> {
    let msg_type = json.get("type").and_then(|t| t.as_str())?;

    match msg_type {
        "system" => {
            let subtype = json.get("subtype").and_then(|t| t.as_str()).unwrap_or("");
            match subtype {
                "hook_started" => {
                    let name = json
                        .get("hook_name")
                        .and_then(|t| t.as_str())
                        .unwrap_or("hook");
                    Some(format!("Running {}...", name))
                }
                "init" => {
                    let model = json
                        .get("model")
                        .and_then(|t| t.as_str())
                        .unwrap_or("model");
                    Some(format!("Connected ({}), thinking...", model))
                }
                _ => None,
            }
        }
        "assistant" => Some("Responding...".to_string()),
        _ => None,
    }
}
