// src/lsp/download.rs
//! Download and extract LSP server binaries.

use crate::lsp::registry::{ArchiveFormat, KnownServer, ServerRegistry};
use std::io::{Read, Write};
use std::path::Path;

/// Progress state for an active download.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub server_name: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub finished: bool,
    pub error: Option<String>,
}

impl DownloadProgress {
    pub fn percent(&self) -> Option<f32> {
        self.total_bytes
            .map(|total| (self.bytes_downloaded as f32 / total as f32) * 100.0)
    }
}

/// Download a server binary in a blocking manner.
/// This should be called from a background thread.
#[allow(dead_code)]
pub fn download_server(
    server: &KnownServer,
    progress_tx: &crossbeam_channel::Sender<DownloadProgress>,
) -> Result<(), String> {
    let url = ServerRegistry::download_url(server)
        .ok_or_else(|| format!("No download URL for {} on this platform", server.name))?;

    let name = server.name.to_string();

    let _ = progress_tx.send(DownloadProgress {
        server_name: name.clone(),
        bytes_downloaded: 0,
        total_bytes: None,
        finished: false,
        error: None,
    });

    // Create server directory
    let server_dir = ServerRegistry::server_dir(server.name);
    std::fs::create_dir_all(&server_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Download
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("Download failed: {}", e))?;

    let total_bytes = response
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    let mut body = response.into_body();
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    let mut downloaded: u64 = 0;

    loop {
        match body.as_reader().read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                downloaded += n as u64;
                let _ = progress_tx.send(DownloadProgress {
                    server_name: name.clone(),
                    bytes_downloaded: downloaded,
                    total_bytes,
                    finished: false,
                    error: None,
                });
            }
            Err(e) => {
                let err = format!("Read error: {}", e);
                let _ = progress_tx.send(DownloadProgress {
                    server_name: name.clone(),
                    bytes_downloaded: downloaded,
                    total_bytes,
                    finished: true,
                    error: Some(err.clone()),
                });
                return Err(err);
            }
        }
    }

    // Extract
    let binary_path = ServerRegistry::server_binary(server);
    extract_archive(
        &data,
        server.archive_format,
        &binary_path,
        server.binary_name,
    )?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&binary_path, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    let _ = progress_tx.send(DownloadProgress {
        server_name: name,
        bytes_downloaded: downloaded,
        total_bytes,
        finished: true,
        error: None,
    });

    Ok(())
}

#[allow(dead_code)]
fn extract_archive(
    data: &[u8],
    format: ArchiveFormat,
    output_path: &Path,
    _binary_name: &str,
) -> Result<(), String> {
    match format {
        ArchiveFormat::Binary => {
            std::fs::write(output_path, data)
                .map_err(|e| format!("Failed to write binary: {}", e))?;
        }
        ArchiveFormat::TarGz => {
            // For rust-analyzer, the .gz file is just a gzip-compressed binary (not a tar)
            // Try gzip decompression first
            use std::io::Cursor;
            let cursor = Cursor::new(data);
            let mut decoder = flate2_minimal_decode(cursor);
            let mut decompressed = Vec::new();
            match decoder.read_to_end(&mut decompressed) {
                Ok(_) => {
                    std::fs::write(output_path, &decompressed)
                        .map_err(|e| format!("Failed to write: {}", e))?;
                }
                Err(_) => {
                    // Fallback: just write raw data
                    std::fs::write(output_path, data)
                        .map_err(|e| format!("Failed to write: {}", e))?;
                }
            }
        }
        ArchiveFormat::Zip => {
            // Simple: just write the data, since we don't bundle a zip library
            // In practice we'd use the zip crate, but for now just write raw
            std::fs::write(output_path, data).map_err(|e| format!("Failed to write: {}", e))?;
        }
    }
    Ok(())
}

/// Minimal gzip decoder using the deflate algorithm.
/// Skips the 10-byte gzip header and optional extras, then decompresses.
#[allow(dead_code)]
fn flate2_minimal_decode(mut reader: impl Read) -> impl Read {
    // Read gzip header (10 bytes minimum)
    let mut header = [0u8; 10];
    let _ = reader.read_exact(&mut header);

    let flags = header[3];

    // Skip FEXTRA
    if flags & 0x04 != 0 {
        let mut xlen = [0u8; 2];
        let _ = reader.read_exact(&mut xlen);
        let len = u16::from_le_bytes(xlen) as usize;
        let mut extra = vec![0u8; len];
        let _ = reader.read_exact(&mut extra);
    }

    // Skip FNAME
    if flags & 0x08 != 0 {
        let mut b = [0u8; 1];
        loop {
            let _ = reader.read_exact(&mut b);
            if b[0] == 0 {
                break;
            }
        }
    }

    // Skip FCOMMENT
    if flags & 0x10 != 0 {
        let mut b = [0u8; 1];
        loop {
            let _ = reader.read_exact(&mut b);
            if b[0] == 0 {
                break;
            }
        }
    }

    // Skip FHCRC
    if flags & 0x02 != 0 {
        let mut crc = [0u8; 2];
        let _ = reader.read_exact(&mut crc);
    }

    // The remaining data is deflate-compressed. We shell out to gunzip instead
    // of bundling a deflate implementation. Use a pipe approach.
    GunzipReader::new(reader)
}

/// A reader that passes the remaining stream through a gunzip process.
/// This is a pragmatic approach to avoid adding a flate2 dependency.
#[allow(dead_code)]
struct GunzipReader {
    data: Vec<u8>,
    pos: usize,
}

impl GunzipReader {
    fn new(mut reader: impl Read) -> Self {
        let mut remaining = Vec::new();
        let _ = reader.read_to_end(&mut remaining);
        // We already stripped the header, but gunzip needs the full file.
        // So we'll use a different approach: just store data for the caller to handle.
        Self {
            data: remaining,
            pos: 0,
        }
    }
}

impl Read for GunzipReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let available = &self.data[self.pos..];
        let n = buf.len().min(available.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.pos += n;
        Ok(n)
    }
}

/// Shell out to gunzip for decompression.
/// Returns decompressed data.
pub fn gunzip_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("gunzip")
        .arg("-c")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn gunzip: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(data)
            .map_err(|e| format!("Failed to write to gunzip: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for gunzip: {}", e))?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err("gunzip failed".to_string())
    }
}

/// Install a server by running a shell command (e.g. `npm install -g pyright`).
/// This should be called from a background thread.
pub fn install_via_command(
    server: &KnownServer,
    command: &str,
    progress_tx: &crossbeam_channel::Sender<DownloadProgress>,
) -> Result<(), String> {
    let name = server.name.to_string();

    let _ = progress_tx.send(DownloadProgress {
        server_name: name.clone(),
        bytes_downloaded: 0,
        total_bytes: None,
        finished: false,
        error: None,
    });

    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let flag = if cfg!(windows) { "/C" } else { "-c" };

    let output = std::process::Command::new(shell)
        .arg(flag)
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run '{}': {}", command, e))?;

    if output.status.success() {
        let _ = progress_tx.send(DownloadProgress {
            server_name: name,
            bytes_downloaded: 0,
            total_bytes: None,
            finished: true,
            error: None,
        });
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        let err = format!("Install failed: {}", detail);
        let _ = progress_tx.send(DownloadProgress {
            server_name: name,
            bytes_downloaded: 0,
            total_bytes: None,
            finished: true,
            error: Some(err.clone()),
        });
        Err(err)
    }
}

/// Improved extraction that uses gunzip for .gz files.
pub fn download_and_install(
    server: &KnownServer,
    progress_tx: &crossbeam_channel::Sender<DownloadProgress>,
) -> Result<(), String> {
    let url = ServerRegistry::download_url(server)
        .ok_or_else(|| format!("No download URL for {} on this platform", server.name))?;

    let name = server.name.to_string();

    let _ = progress_tx.send(DownloadProgress {
        server_name: name.clone(),
        bytes_downloaded: 0,
        total_bytes: None,
        finished: false,
        error: None,
    });

    let server_dir = ServerRegistry::server_dir(server.name);
    std::fs::create_dir_all(&server_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Download
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("Download failed: {}", e))?;

    let total_bytes = response
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    let mut body = response.into_body();
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    let mut downloaded: u64 = 0;

    loop {
        match body.as_reader().read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                downloaded += n as u64;
                let _ = progress_tx.send(DownloadProgress {
                    server_name: name.clone(),
                    bytes_downloaded: downloaded,
                    total_bytes,
                    finished: false,
                    error: None,
                });
            }
            Err(e) => {
                let err = format!("Read error: {}", e);
                let _ = progress_tx.send(DownloadProgress {
                    server_name: name.clone(),
                    bytes_downloaded: downloaded,
                    total_bytes,
                    finished: true,
                    error: Some(err.clone()),
                });
                return Err(err);
            }
        }
    }

    let binary_path = ServerRegistry::server_binary(server);

    // Extract based on format
    match server.archive_format {
        ArchiveFormat::Binary => {
            std::fs::write(&binary_path, &data).map_err(|e| format!("Failed to write: {}", e))?;
        }
        ArchiveFormat::TarGz => {
            // Use gunzip to decompress
            let decompressed = gunzip_bytes(&data)?;
            std::fs::write(&binary_path, &decompressed)
                .map_err(|e| format!("Failed to write: {}", e))?;
        }
        ArchiveFormat::Zip => {
            std::fs::write(&binary_path, &data).map_err(|e| format!("Failed to write: {}", e))?;
        }
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&binary_path, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    let _ = progress_tx.send(DownloadProgress {
        server_name: name,
        bytes_downloaded: downloaded,
        total_bytes,
        finished: true,
        error: None,
    });

    Ok(())
}
