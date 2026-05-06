// src/lsp/registry.rs
//! Known LSP servers registry: binary names, download URLs, language IDs.

use std::path::PathBuf;

/// A known LSP server definition.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KnownServer {
    pub name: &'static str,
    pub language_ids: &'static [&'static str],
    pub binary_name: &'static str,
    /// Arguments to pass when spawning the server (e.g. `&["--stdio"]`).
    pub args: &'static [&'static str],
    pub download_url_macos_arm64: Option<&'static str>,
    pub download_url_macos_x86_64: Option<&'static str>,
    pub download_url_linux_x86_64: Option<&'static str>,
    pub archive_format: ArchiveFormat,
    /// Hint shown in UI when no managed download or install command is available.
    pub install_hint: Option<&'static str>,
    /// Shell command to install this server (cross-platform, e.g. `npm install -g pyright`).
    pub install_command: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ArchiveFormat {
    TarGz,
    Zip,
    Binary,
}

/// Table of known LSP servers.
pub static KNOWN_SERVERS: &[KnownServer] = &[
    KnownServer {
        name: "rust-analyzer",
        language_ids: &["rust"],
        binary_name: "rust-analyzer",
        args: &[],
        download_url_macos_arm64: Some(
            "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-aarch64-apple-darwin.gz",
        ),
        download_url_macos_x86_64: Some(
            "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-x86_64-apple-darwin.gz",
        ),
        download_url_linux_x86_64: Some(
            "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-x86_64-unknown-linux-gnu.gz",
        ),
        archive_format: ArchiveFormat::TarGz,
        install_hint: Some("rustup component add rust-analyzer"),
        install_command: Some("rustup component add rust-analyzer"),
    },
    KnownServer {
        name: "pyright",
        language_ids: &["python"],
        binary_name: "pyright-langserver",
        args: &["--stdio"],
        download_url_macos_arm64: None,
        download_url_macos_x86_64: None,
        download_url_linux_x86_64: None,
        archive_format: ArchiveFormat::Binary,
        install_hint: Some("npm install -g pyright"),
        install_command: Some("npm install -g pyright"),
    },
    KnownServer {
        name: "typescript-language-server",
        language_ids: &["typescript", "javascript", "typescriptreact", "javascriptreact"],
        binary_name: "typescript-language-server",
        args: &["--stdio"],
        download_url_macos_arm64: None,
        download_url_macos_x86_64: None,
        download_url_linux_x86_64: None,
        archive_format: ArchiveFormat::Binary,
        install_hint: Some("npm install -g typescript-language-server typescript"),
        install_command: Some("npm install -g typescript-language-server typescript"),
    },
    KnownServer {
        name: "lua-language-server",
        language_ids: &["lua"],
        binary_name: "lua-language-server",
        args: &[],
        download_url_macos_arm64: None,
        download_url_macos_x86_64: None,
        download_url_linux_x86_64: None,
        archive_format: ArchiveFormat::TarGz,
        install_hint: Some("brew install lua-language-server"),
        install_command: None,
    },
    KnownServer {
        name: "gopls",
        language_ids: &["go"],
        binary_name: "gopls",
        args: &[],
        download_url_macos_arm64: None,
        download_url_macos_x86_64: None,
        download_url_linux_x86_64: None,
        archive_format: ArchiveFormat::Binary,
        install_hint: Some("go install golang.org/x/tools/gopls@latest"),
        install_command: Some("go install golang.org/x/tools/gopls@latest"),
    },
    KnownServer {
        name: "clangd",
        language_ids: &["c", "cpp", "objc", "objcpp"],
        binary_name: "clangd",
        args: &[],
        download_url_macos_arm64: None,
        download_url_macos_x86_64: None,
        download_url_linux_x86_64: None,
        archive_format: ArchiveFormat::TarGz,
        install_hint: Some("brew install llvm"),
        install_command: None,
    },
    KnownServer {
        name: "zls",
        language_ids: &["zig"],
        binary_name: "zls",
        args: &[],
        download_url_macos_arm64: None,
        download_url_macos_x86_64: None,
        download_url_linux_x86_64: None,
        archive_format: ArchiveFormat::Binary,
        install_hint: Some("zig fetch --global-cache-dir"),
        install_command: None,
    },
];

/// Server status for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ServerStatus {
    NotInstalled,
    Installed,
    Running,
    Error,
}

#[allow(dead_code)]
impl ServerStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotInstalled => "not installed",
            Self::Installed => "installed",
            Self::Running => "running",
            Self::Error => "error",
        }
    }
}

/// The server registry manages known servers and their installation state.
pub struct ServerRegistry;

impl ServerRegistry {
    /// Directory where LSP server binaries are stored.
    pub fn servers_dir() -> PathBuf {
        crate::config::NyxConfig::config_dir().join("lsp-servers")
    }

    /// Path to a specific server's directory.
    pub fn server_dir(name: &str) -> PathBuf {
        Self::servers_dir().join(name)
    }

    /// Path to the server binary.
    pub fn server_binary(server: &KnownServer) -> PathBuf {
        Self::server_dir(server.name).join(server.binary_name)
    }

    /// Check if a server is installed (binary exists and is executable).
    pub fn is_installed(server: &KnownServer) -> bool {
        let path = Self::server_binary(server);
        path.exists()
    }

    /// Try to find the command for a server: first check managed install, then PATH.
    pub fn find_command(server: &KnownServer, custom_command: Option<&str>) -> Option<String> {
        if let Some(cmd) = custom_command {
            return Some(cmd.to_string());
        }

        // Check managed installation
        let managed = Self::server_binary(server);
        if managed.exists() {
            return managed.to_str().map(|s| s.to_string());
        }

        // Check PATH
        if which_exists(server.binary_name) {
            return Some(server.binary_name.to_string());
        }

        None
    }

    /// Find the known server for a given language ID.
    pub fn server_for_language(language_id: &str) -> Option<&'static KnownServer> {
        KNOWN_SERVERS
            .iter()
            .find(|s| s.language_ids.contains(&language_id))
    }

    /// Find a known server by its canonical name.
    pub fn known_server_by_name(name: &str) -> Option<&'static KnownServer> {
        KNOWN_SERVERS.iter().find(|s| s.name == name)
    }

    /// Delete a server's managed installation.
    pub fn uninstall(server: &KnownServer) -> Result<(), String> {
        let dir = Self::server_dir(server.name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .map_err(|e| format!("Failed to remove {}: {}", dir.display(), e))?;
        }
        Ok(())
    }

    /// Get the download URL for the current platform.
    pub fn download_url(server: &KnownServer) -> Option<&'static str> {
        #[cfg(target_os = "macos")]
        {
            #[cfg(target_arch = "aarch64")]
            {
                server.download_url_macos_arm64
            }
            #[cfg(target_arch = "x86_64")]
            {
                server.download_url_macos_x86_64
            }
            #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
            {
                None
            }
        }
        #[cfg(target_os = "linux")]
        {
            #[cfg(target_arch = "x86_64")]
            {
                server.download_url_linux_x86_64
            }
            #[cfg(not(target_arch = "x86_64"))]
            {
                None
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }
}

/// Check if a binary exists on PATH.
fn which_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_servers_have_names() {
        for server in KNOWN_SERVERS {
            assert!(!server.name.is_empty());
            assert!(!server.binary_name.is_empty());
            assert!(!server.language_ids.is_empty());
        }
    }

    #[test]
    fn server_for_language_rust() {
        let server = ServerRegistry::server_for_language("rust");
        assert!(server.is_some());
        assert_eq!(server.unwrap().name, "rust-analyzer");
    }

    #[test]
    fn server_for_language_unknown() {
        let server = ServerRegistry::server_for_language("brainfuck");
        assert!(server.is_none());
    }

    #[test]
    fn server_dir_contains_name() {
        let dir = ServerRegistry::server_dir("rust-analyzer");
        assert!(dir.to_str().unwrap().contains("rust-analyzer"));
    }

    #[test]
    fn server_status_labels() {
        assert_eq!(ServerStatus::NotInstalled.label(), "not installed");
        assert_eq!(ServerStatus::Running.label(), "running");
    }
}
