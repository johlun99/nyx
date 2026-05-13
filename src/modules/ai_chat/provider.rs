use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

pub enum ProviderRequest {
    SendMessage {
        prompt: String,
        context: Option<ChatContext>,
    },
    Cancel,
    Shutdown,
}

pub enum ProviderEvent {
    Token(String),
    Status(String),
    Done {
        token_usage: Option<usize>,
        context_window: Option<usize>,
    },
    Error(String),
}

pub struct ChatContext {
    pub file_path: Option<String>,
    pub file_content: Option<String>,
    pub selection: Option<String>,
    pub working_directory: Option<PathBuf>,
}

pub struct ProviderHandle {
    pub request_tx: Sender<ProviderRequest>,
    pub event_rx: Receiver<ProviderEvent>,
    #[allow(dead_code)]
    pub config: ProviderConfig,
}
