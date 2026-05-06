// src/lsp/mod.rs
//! LSP integration: manages language server clients, diagnostics, and completions.

pub mod client;
pub mod download;
pub mod protocol;
pub mod registry;
pub mod transport;

use crate::config::lsp_config::LspConfig;
use crate::lsp::client::{spawn_client, LspClientHandle, LspRequest, LspResponse};
use crate::lsp::download::{download_and_install, install_via_command, DownloadProgress};
use crate::lsp::protocol::{
    char_offset_to_lsp_position, lsp_position_to_char_offset, CompletionItem, Diagnostic,
    DiagnosticSeverity, ServerCapabilities,
};
use crate::lsp::registry::{KnownServer, ServerRegistry, ServerStatus};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

/// State for an active completion popup.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CompletionState {
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub anchor_line: usize,
    pub anchor_col: usize,
    pub filter_text: String,
}

impl CompletionState {
    pub fn filtered_items(&self) -> Vec<&CompletionItem> {
        if self.filter_text.is_empty() {
            return self.items.iter().collect();
        }
        let filter = self.filter_text.to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                let text = item
                    .filter_text
                    .as_deref()
                    .unwrap_or(&item.label)
                    .to_lowercase();
                text.contains(&filter) && text != filter
            })
            .collect()
    }

    pub fn move_selection(&mut self, delta: i32) {
        let filtered_len = self.filtered_items().len();
        if filtered_len == 0 {
            return;
        }
        let new = self.selected as i32 + delta;
        self.selected = if new < 0 {
            0
        } else if new >= filtered_len as i32 {
            filtered_len - 1
        } else {
            new as usize
        };
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        let filtered = self.filtered_items();
        filtered.get(self.selected).copied()
    }
}

/// Nyx's diagnostic representation (resolved to char offsets).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NyxDiagnostic {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

/// The main LSP manager, owned by NyxApp.
pub struct LspManager {
    clients: HashMap<String, LspClientHandle>,
    pub diagnostics: HashMap<String, Vec<NyxDiagnostic>>,
    pub completion: Option<CompletionState>,
    pub lsp_config: LspConfig,
    /// Debounce: last document change time per URI
    pending_changes: HashMap<String, (Instant, String, i32)>,
    document_versions: HashMap<String, i32>,
    document_texts: HashMap<String, String>,
    open_documents_by_server: HashMap<String, HashSet<String>>,
    /// Download progress (from background thread)
    download_progress: Option<DownloadProgress>,
    download_progress_rx: Option<crossbeam_channel::Receiver<DownloadProgress>>,
    /// Last server error message (shown in UI)
    pub last_error: Option<String>,
}

impl LspManager {
    pub fn new(lsp_config: LspConfig) -> Self {
        Self {
            clients: HashMap::new(),
            diagnostics: HashMap::new(),
            completion: None,
            lsp_config,
            pending_changes: HashMap::new(),
            document_versions: HashMap::new(),
            document_texts: HashMap::new(),
            open_documents_by_server: HashMap::new(),
            download_progress: None,
            download_progress_rx: None,
            last_error: None,
        }
    }

    /// Poll all active clients for responses. Call once per frame.
    /// Returns true if a download just completed successfully (caller should retry server start).
    pub fn poll(&mut self, buffer_text: Option<&str>) -> bool {
        // Flush debounced changes
        self.flush_pending_changes();

        // Poll download progress
        let mut download_completed = false;
        if let Some(ref rx) = self.download_progress_rx {
            while let Ok(progress) = rx.try_recv() {
                let finished = progress.finished;
                let success = finished && progress.error.is_none();
                self.download_progress = Some(progress);
                if finished {
                    self.download_progress_rx = None;
                    download_completed = success;
                    break;
                }
            }
        }

        // Poll client responses
        let client_names: Vec<String> = self.clients.keys().cloned().collect();
        for name in client_names {
            let responses: Vec<LspResponse> = {
                if let Some(client) = self.clients.get(&name) {
                    let mut resps = Vec::new();
                    while let Ok(resp) = client.response_rx.try_recv() {
                        resps.push(resp);
                    }
                    resps
                } else {
                    continue;
                }
            };

            for response in responses {
                match response {
                    LspResponse::Initialized(caps) => {
                        if let Some(client) = self.clients.get_mut(&name) {
                            client.capabilities = Some(caps);
                        }
                        tracing::info!("LSP server {} initialized", name);
                    }
                    LspResponse::Diagnostics { uri, diagnostics } => {
                        let uri = normalize_uri(&uri);
                        tracing::info!(
                            "LSP: received {} diagnostics for {}",
                            diagnostics.len(),
                            uri
                        );
                        let nyx_diags: Vec<NyxDiagnostic> = if let Some(text) = self
                            .document_texts
                            .get(&uri)
                            .map(|s| s.as_str())
                            .or(buffer_text)
                        {
                            diagnostics
                                .iter()
                                .map(|d| resolve_diagnostic(d, text))
                                .collect()
                        } else {
                            Vec::new()
                        };
                        self.diagnostics.insert(uri, nyx_diags);
                    }
                    LspResponse::Completions(items) => {
                        tracing::info!("LSP: received {} completion items", items.len());
                        if let Some(ref mut state) = self.completion {
                            state.items = items;
                            state.selected = 0;
                        }
                    }
                    LspResponse::ServerError(msg) => {
                        tracing::warn!("LSP server {} error: {}", name, msg);
                        let with_hint = ServerRegistry::known_server_by_name(&name)
                            .and_then(|s| s.install_hint)
                            .map(|hint| format!("{}: {} (install: {})", name, msg, hint))
                            .unwrap_or_else(|| format!("{}: {}", name, msg));
                        self.last_error = Some(with_hint);
                    }
                    LspResponse::ServerStopped => {
                        tracing::info!("LSP server {} stopped", name);
                        self.clients.remove(&name);
                    }
                }
            }
        }

        download_completed
    }

    /// Notify that a document was opened.
    pub fn notify_document_open(&mut self, file_path: &str, language_id: &str, text: &str) {
        let uri = path_to_uri(file_path);
        let version = 1;
        self.document_versions.insert(uri.clone(), version);
        self.document_texts.insert(uri.clone(), text.to_string());

        // Find or start the appropriate server
        if let Some(server_name) = self.ensure_server_for_language(language_id) {
            if let Some(client) = self.clients.get(&server_name) {
                tracing::info!("LSP: sending didOpen for {} to {}", file_path, server_name);
                let _ = client.request_tx.send(LspRequest::DidOpen {
                    uri,
                    language_id: language_id.to_string(),
                    version,
                    text: text.to_string(),
                });
                self.mark_document_open_for_server(&server_name, &path_to_uri(file_path));
            }
        }
    }

    /// Notify that a document changed. Debounced: actual send happens in poll().
    pub fn notify_document_change(&mut self, file_path: &str, text: &str) {
        let uri = path_to_uri(file_path);
        let version = self
            .document_versions
            .entry(uri.clone())
            .and_modify(|v| *v += 1)
            .or_insert(1);
        self.pending_changes
            .insert(uri, (Instant::now(), text.to_string(), *version));
        self.document_texts
            .insert(path_to_uri(file_path), text.to_string());
        // Optimistic clear to avoid stale diagnostics lingering after local edits.
        self.diagnostics.remove(&path_to_uri(file_path));
    }

    /// Notify that a document was saved.
    pub fn notify_document_save(&mut self, file_path: &str, text: &str) {
        let uri = path_to_uri(file_path);
        self.document_texts.insert(uri.clone(), text.to_string());
        for client in self.clients.values() {
            let _ = client.request_tx.send(LspRequest::DidSave {
                uri: uri.clone(),
                text: text.to_string(),
            });
        }
    }

    /// Notify that a document was closed.
    #[allow(dead_code)]
    pub fn notify_document_close(&mut self, file_path: &str) {
        let uri = path_to_uri(file_path);
        self.pending_changes.remove(&uri);
        self.diagnostics.remove(&uri);
        self.document_versions.remove(&uri);
        self.document_texts.remove(&uri);
        for uris in self.open_documents_by_server.values_mut() {
            uris.remove(&uri);
        }

        for client in self.clients.values() {
            let _ = client
                .request_tx
                .send(LspRequest::DidClose { uri: uri.clone() });
        }
    }

    /// Request completions at the current cursor position.
    pub fn request_completion(
        &mut self,
        file_path: &str,
        text: &str,
        cursor_line: usize,
        cursor_col: usize,
    ) {
        let uri = path_to_uri(file_path);
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if let Some(lang_id) = crate::syntax::languages::language_for_extension(ext) {
            let _ = self.ensure_server_for_language(lang_id);
        }

        if self.clients.is_empty() {
            self.last_error = Some("No active LSP server for this file".to_string());
            self.completion = None;
            return;
        }

        // Calculate LSP position
        let char_offset = {
            let mut offset = 0;
            for (i, line) in text.lines().enumerate() {
                if i == cursor_line {
                    offset += cursor_col;
                    break;
                }
                offset += line.chars().count() + 1; // +1 for \n
            }
            offset
        };
        let position = char_offset_to_lsp_position(text, char_offset);

        let anchor_col = find_completion_anchor_col(text, cursor_line, cursor_col);
        let filter_text = completion_filter_text(text, cursor_line, anchor_col, cursor_col);

        self.completion = Some(CompletionState {
            items: Vec::new(),
            selected: 0,
            anchor_line: cursor_line,
            anchor_col,
            filter_text,
        });

        // Select clients that advertise completion; if capabilities are not known yet,
        // fall back to all clients to avoid dropping requests during startup races.
        let supports: Vec<String> = self
            .clients
            .iter()
            .filter(|(_, c)| client_supports_completion(c))
            .map(|(name, _)| name.clone())
            .collect();
        let targets: Vec<String> = if supports.is_empty() {
            self.clients.keys().cloned().collect()
        } else {
            supports
        };

        let language_id = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(crate::syntax::languages::language_for_extension)
            .unwrap_or("plaintext")
            .to_string();
        let version = *self.document_versions.entry(uri.clone()).or_insert(1);

        for server_name in targets {
            if let Some(client) = self.clients.get(&server_name) {
                let request_tx = client.request_tx.clone();
                if !self.is_document_open_for_server(&server_name, &uri) {
                    let _ = request_tx.send(LspRequest::DidOpen {
                        uri: uri.clone(),
                        language_id: language_id.clone(),
                        version,
                        text: text.to_string(),
                    });
                    self.mark_document_open_for_server(&server_name, &uri);
                }
                let _ = request_tx.send(LspRequest::Completion {
                    uri: uri.clone(),
                    position,
                });
            }
        }
    }

    /// Dismiss the completion popup.
    pub fn dismiss_completion(&mut self) {
        self.completion = None;
    }

    /// Accept the currently selected completion.
    /// Returns the text to insert and the range to replace (anchor_col..current_col).
    pub fn accept_completion(&mut self) -> Option<(String, usize, usize)> {
        let state = self.completion.take()?;
        let item = state.selected_item()?.clone();
        let text = item.text_to_insert().to_string();
        Some((
            text,
            state.anchor_col,
            state.anchor_col + state.filter_text.len(),
        ))
    }

    /// Start (or ensure running) the LSP server for a given language.
    /// Returns the server name if successful.
    fn ensure_server_for_language(&mut self, language_id: &str) -> Option<String> {
        // Find which known server handles this language
        let server = ServerRegistry::server_for_language(language_id)?;

        // Already running?
        if self.clients.contains_key(server.name) {
            return Some(server.name.to_string());
        }

        // Enabled in config?
        if !self.lsp_config.is_enabled(server.name) {
            tracing::debug!("LSP: server '{}' is not enabled, skipping", server.name);
            return None;
        }

        // Find the command
        let custom_cmd = self.lsp_config.custom_command(server.name);
        let command = match ServerRegistry::find_command(server, custom_cmd) {
            Some(cmd) => cmd,
            None => {
                tracing::debug!("LSP: binary for '{}' not found", server.name);
                return None;
            }
        };

        // Determine root URI from CWD
        let root_uri = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(path_to_uri))
            .unwrap_or_else(|| "file:///".to_string());

        tracing::info!(
            "LSP: starting server '{}' with command '{}'",
            server.name,
            command
        );
        match spawn_client(server.name, &command, server.args, &root_uri) {
            Ok(handle) => {
                let name = server.name.to_string();
                self.clients.insert(name.clone(), handle);
                tracing::info!("LSP: server '{}' spawned successfully", name);
                Some(name)
            }
            Err(e) => {
                tracing::error!("LSP: failed to start server '{}': {}", server.name, e);
                self.last_error = Some(format!("{}: {}", server.name, e));
                None
            }
        }
    }

    /// Flush pending document changes that have been debounced for >=300ms.
    fn flush_pending_changes(&mut self) {
        let now = Instant::now();
        let debounce = std::time::Duration::from_millis(300);

        let ready: Vec<(String, String, i32)> = self
            .pending_changes
            .iter()
            .filter(|(_, (time, _, _))| now.duration_since(*time) >= debounce)
            .map(|(uri, (_, text, version))| (uri.clone(), text.clone(), *version))
            .collect();

        for (uri, text, version) in ready {
            self.pending_changes.remove(&uri);
            for client in self.clients.values() {
                let _ = client.request_tx.send(LspRequest::DidChange {
                    uri: uri.clone(),
                    version,
                    text: text.clone(),
                });
            }
        }
    }

    /// Get the status of a known server.
    pub fn server_status(&self, server: &KnownServer) -> ServerStatus {
        if self.clients.contains_key(server.name) {
            return ServerStatus::Running;
        }
        if ServerRegistry::is_installed(server) {
            return ServerStatus::Installed;
        }
        // Check if available on PATH
        let custom = self.lsp_config.custom_command(server.name);
        if ServerRegistry::find_command(server, custom).is_some() {
            return ServerStatus::Installed;
        }
        ServerStatus::NotInstalled
    }

    /// Start downloading a server binary in the background.
    pub fn start_download(&mut self, server: &'static KnownServer) {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.download_progress_rx = Some(rx);

        std::thread::Builder::new()
            .name(format!("lsp-download-{}", server.name))
            .spawn(move || {
                if let Err(e) = download_and_install(server, &tx) {
                    let _ = tx.send(DownloadProgress {
                        server_name: server.name.to_string(),
                        bytes_downloaded: 0,
                        total_bytes: None,
                        finished: true,
                        error: Some(e),
                    });
                }
            })
            .expect("spawn download thread");
    }

    /// Start installing a server via shell command in a background thread.
    pub fn start_install(&mut self, server: &'static KnownServer, command: &str) {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.download_progress_rx = Some(rx);

        let cmd = command.to_string();
        std::thread::Builder::new()
            .name(format!("lsp-install-{}", server.name))
            .spawn(move || {
                if let Err(e) = install_via_command(server, &cmd, &tx) {
                    let _ = tx.send(DownloadProgress {
                        server_name: server.name.to_string(),
                        bytes_downloaded: 0,
                        total_bytes: None,
                        finished: true,
                        error: Some(e),
                    });
                }
            })
            .expect("spawn install thread");
    }

    /// Get current download progress (if any).
    pub fn download_progress(&self) -> Option<&DownloadProgress> {
        self.download_progress.as_ref()
    }

    /// Clear download progress after user has seen the result.
    #[allow(dead_code)]
    pub fn clear_download_progress(&mut self) {
        self.download_progress = None;
    }

    /// Shutdown a specific server by name.
    #[allow(dead_code)]
    pub fn shutdown_server(&mut self, server_name: &str) {
        if let Some(client) = self.clients.get(server_name) {
            let _ = client.request_tx.send(LspRequest::Shutdown);
        }
        self.clients.remove(server_name);
    }

    /// Shutdown all running servers.
    pub fn shutdown_all(&mut self) {
        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in names {
            if let Some(client) = self.clients.get(&name) {
                let _ = client.request_tx.send(LspRequest::Shutdown);
            }
        }
    }

    /// Get diagnostics for a specific file.
    pub fn diagnostics_for_file(&self, file_path: &str) -> &[NyxDiagnostic] {
        let uri = path_to_uri(file_path);
        self.diagnostics
            .get(&uri)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Count errors and warnings for the current file.
    pub fn diagnostic_counts(&self, file_path: &str) -> (usize, usize) {
        let diags = self.diagnostics_for_file(file_path);
        let errors = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::ERROR)
            .count();
        let warnings = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::WARNING)
            .count();
        (errors, warnings)
    }

    /// Save LSP config to disk.
    pub fn save_config(&self) {
        let _ = self.lsp_config.save(&LspConfig::config_path());
    }

    /// Check if any LSP server is currently running.
    #[allow(dead_code)]
    pub fn has_running_servers(&self) -> bool {
        !self.clients.is_empty()
    }

    /// Check if there are any active LSP clients.
    pub fn has_clients(&self) -> bool {
        !self.clients.is_empty()
    }

    /// Human-readable health status for the active file's language server.
    pub fn health_summary_for_file(&self, file_path: &str) -> Option<String> {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let language_id = crate::syntax::languages::language_for_extension(ext)?;
        let server = ServerRegistry::server_for_language(language_id)?;

        if !self.lsp_config.is_enabled(server.name) {
            return Some(format!("LSP {}: disabled", server.name));
        }

        let status = self.server_status(server);
        let base = match status {
            ServerStatus::Running => format!("LSP {}: running", server.name),
            ServerStatus::Installed => format!("LSP {}: installed (not running)", server.name),
            ServerStatus::NotInstalled => {
                if let Some(hint) = server.install_hint {
                    format!("LSP {}: missing ({})", server.name, hint)
                } else {
                    format!("LSP {}: missing", server.name)
                }
            }
            ServerStatus::Error => format!("LSP {}: error", server.name),
        };

        if let Some(err) = self.last_error.as_deref() {
            if err.starts_with(server.name) {
                return Some(format!("{} | {}", base, err));
            }
        }
        Some(base)
    }

    fn is_document_open_for_server(&self, server_name: &str, uri: &str) -> bool {
        self.open_documents_by_server
            .get(server_name)
            .map(|uris| uris.contains(uri))
            .unwrap_or(false)
    }

    fn mark_document_open_for_server(&mut self, server_name: &str, uri: &str) {
        self.open_documents_by_server
            .entry(server_name.to_string())
            .or_default()
            .insert(uri.to_string());
    }
}

fn find_completion_anchor_col(text: &str, cursor_line: usize, cursor_col: usize) -> usize {
    let line = text.lines().nth(cursor_line).unwrap_or("");
    let chars: Vec<char> = line.chars().collect();
    let mut col = cursor_col.min(chars.len());
    while col > 0 {
        let ch = chars[col - 1];
        if ch.is_alphanumeric() || ch == '_' {
            col -= 1;
        } else {
            break;
        }
    }
    col
}

fn completion_filter_text(
    text: &str,
    cursor_line: usize,
    anchor_col: usize,
    cursor_col: usize,
) -> String {
    let line = text.lines().nth(cursor_line).unwrap_or("");
    line.chars()
        .skip(anchor_col)
        .take(cursor_col.saturating_sub(anchor_col))
        .collect()
}

fn client_supports_completion(client: &LspClientHandle) -> bool {
    client
        .capabilities
        .as_ref()
        .map(|caps: &ServerCapabilities| caps.completion_provider)
        .unwrap_or(false)
}

/// Convert a file path to a file:// URI.
fn path_to_uri(path: &str) -> String {
    normalize_uri(path)
}

fn normalize_uri(input: &str) -> String {
    let path_part = if let Some(raw) = input.strip_prefix("file://") {
        percent_decode(raw)
    } else {
        input.to_string()
    };

    let abs = if std::path::Path::new(&path_part).is_absolute() {
        std::path::PathBuf::from(&path_part)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&path_part))
            .unwrap_or_else(|_| std::path::PathBuf::from(&path_part))
    };

    let canonical = abs.canonicalize().unwrap_or(abs);
    format!("file://{}", canonical.to_string_lossy())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = bytes[i + 1] as char;
            let h2 = bytes[i + 2] as char;
            if let (Some(a), Some(b)) = (h1.to_digit(16), h2.to_digit(16)) {
                out.push(((a << 4) | b) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Resolve an LSP Diagnostic to char-based offsets.
fn resolve_diagnostic(diag: &Diagnostic, text: &str) -> NyxDiagnostic {
    let start_offset = lsp_position_to_char_offset(text, diag.range.start);
    let end_offset = lsp_position_to_char_offset(text, diag.range.end);

    // Convert offsets back to line/col
    let (start_line, start_col) = offset_to_line_col(text, start_offset);
    let (end_line, end_col) = offset_to_line_col(text, end_offset);

    NyxDiagnostic {
        start_line,
        start_col,
        end_line,
        end_col,
        severity: diag.severity.unwrap_or(DiagnosticSeverity::ERROR),
        message: diag.message.clone(),
    }
}

fn offset_to_line_col(text: &str, char_offset: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in text.chars().enumerate() {
        if i >= char_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_uri_conversion() {
        assert_eq!(path_to_uri("/foo/bar.rs"), "file:///foo/bar.rs");
        assert_eq!(path_to_uri("file:///foo/bar.rs"), "file:///foo/bar.rs");
    }

    #[test]
    fn offset_to_line_col_basic() {
        let text = "hello\nworld";
        assert_eq!(offset_to_line_col(text, 0), (0, 0));
        assert_eq!(offset_to_line_col(text, 5), (0, 5));
        assert_eq!(offset_to_line_col(text, 6), (1, 0));
        assert_eq!(offset_to_line_col(text, 8), (1, 2));
    }

    #[test]
    fn completion_state_filtering() {
        let state = CompletionState {
            items: vec![
                CompletionItem {
                    label: "println".to_string(),
                    kind: None,
                    detail: None,
                    insert_text: None,
                    filter_text: None,
                    sort_text: None,
                },
                CompletionItem {
                    label: "print".to_string(),
                    kind: None,
                    detail: None,
                    insert_text: None,
                    filter_text: None,
                    sort_text: None,
                },
                CompletionItem {
                    label: "eprintln".to_string(),
                    kind: None,
                    detail: None,
                    insert_text: None,
                    filter_text: None,
                    sort_text: None,
                },
            ],
            selected: 0,
            anchor_line: 0,
            anchor_col: 0,
            filter_text: "print".to_string(),
        };

        let filtered = state.filtered_items();
        assert_eq!(filtered.len(), 2); // "println" and "eprintln" (exact match "print" excluded)
    }

    #[test]
    fn completion_state_move_selection() {
        let mut state = CompletionState {
            items: vec![
                CompletionItem {
                    label: "a".into(),
                    kind: None,
                    detail: None,
                    insert_text: None,
                    filter_text: None,
                    sort_text: None,
                },
                CompletionItem {
                    label: "b".into(),
                    kind: None,
                    detail: None,
                    insert_text: None,
                    filter_text: None,
                    sort_text: None,
                },
            ],
            selected: 0,
            anchor_line: 0,
            anchor_col: 0,
            filter_text: String::new(),
        };

        state.move_selection(1);
        assert_eq!(state.selected, 1);
        state.move_selection(1); // should clamp
        assert_eq!(state.selected, 1);
        state.move_selection(-1);
        assert_eq!(state.selected, 0);
        state.move_selection(-1); // should clamp
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn diagnostic_counts_empty() {
        let config = LspConfig::default();
        let manager = LspManager::new(config);
        assert_eq!(manager.diagnostic_counts("/nonexistent"), (0, 0));
    }
}
