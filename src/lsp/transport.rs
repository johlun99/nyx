// src/lsp/transport.rs
//! JSON-RPC framing over stdin/stdout (LSP base protocol).

use serde_json::Value;
use std::io::{self, BufRead, BufReader, Read, Write};

/// Write a JSON-RPC message with Content-Length header.
pub fn write_message<W: Write>(writer: &mut W, body: &Value) -> io::Result<()> {
    let content =
        serde_json::to_string(body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    writer.write_all(header.as_bytes())?;
    writer.write_all(content.as_bytes())?;
    writer.flush()
}

/// Read a JSON-RPC message from a buffered reader.
/// Blocks until a complete message is available or EOF.
pub fn read_message<R: Read>(reader: &mut BufReader<R>) -> io::Result<Value> {
    let mut content_length: Option<usize> = None;

    // Read headers
    loop {
        let mut header_line = String::new();
        let bytes_read = reader.read_line(&mut header_line)?;
        if bytes_read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF reading headers",
            ));
        }

        let trimmed = header_line.trim();
        if trimmed.is_empty() {
            // End of headers
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
            content_length = value.parse().ok();
        }
        // Ignore other headers (Content-Type, etc.)
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Missing Content-Length header")
    })?;

    // Read body
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;

    serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_message() {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "test",
            "params": { "hello": "world" }
        });

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let parsed = read_message(&mut reader).unwrap();

        assert_eq!(parsed["method"], "test");
        assert_eq!(parsed["params"]["hello"], "world");
    }

    #[test]
    fn write_message_format() {
        let msg = serde_json::json!({"id": 1});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("Content-Length: "));
        assert!(output.contains("\r\n\r\n"));
    }

    #[test]
    fn read_missing_content_length() {
        let data = b"\r\n{\"id\":1}";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader);
        assert!(result.is_err());
    }
}
