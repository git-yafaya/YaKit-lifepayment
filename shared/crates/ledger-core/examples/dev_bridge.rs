//! 仅用于本机浏览器验收，账本为临时文件，正式桌面包不包含此入口。
use lightledger_core::LedgerService;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("lightledger-browser-{}.db", std::process::id()));
    let mut app = LedgerService::open(&path, &[19u8; 32])?;
    let listener = TcpListener::bind("127.0.0.1:1421")?;
    println!(
        "开发验收桥: http://127.0.0.1:1421，临时账本 {}",
        path.display()
    );
    for stream in listener.incoming() {
        let mut stream = stream?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        let mut bytes = vec![];
        let mut buffer = [0u8; 4096];
        let (header_end, length) = loop {
            let n = stream.read(&mut buffer)?;
            if n == 0 {
                break (0, 0);
            }
            bytes.extend_from_slice(&buffer[..n]);
            if bytes.len() > 2_000_000 {
                break (0, 0);
            }
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                break (end + 4, length);
            }
        };
        if header_end == 0 {
            continue;
        }
        while bytes.len() < header_end + length {
            let n = stream.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
        }
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let origin = headers
            .lines()
            .find_map(|l| {
                l.strip_prefix("Origin: ")
                    .or_else(|| l.strip_prefix("origin: "))
            })
            .unwrap_or("");
        if !origin.is_empty() && origin != "http://127.0.0.1:1420" {
            let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n");
            continue;
        }
        let value = if headers.starts_with("OPTIONS ") {
            json!({})
        } else {
            match serde_json::from_slice::<Value>(&bytes[header_end..]) {
                Ok(v) => {
                    match app.dispatch(v["action"].as_str().unwrap_or(""), v["payload"].clone()) {
                        Ok(value) => json!({"ok":true,"value":value}),
                        Err(e) => json!({"ok":false,"error":e}),
                    }
                }
                Err(e) => json!({"ok":false,"error":e.to_string()}),
            }
        };
        let body = value.to_string();
        let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: http://127.0.0.1:1420\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",body.len(),body);
        let _ = stream.write_all(response.as_bytes());
    }
    Ok(())
}
