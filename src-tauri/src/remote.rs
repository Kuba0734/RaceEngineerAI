//! Read-only dashboard for a phone or second screen in the local network. A tiny HTTP server
//! (no cloud, off by default): `/` serves one self-contained page, `/live` streams the live
//! frame as server-sent events. Optional PIN in the query string.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::core::Core;

const PAGE: &str = include_str!("../resources/remote/index.html");
/// Updates per second sent to the browser.
const RATE_HZ: u64 = 5;
/// Every n-th update carries the heavy parts (standings, relative, fuel).
const HEAVY_EVERY: u64 = 5;
const MAX_REQUEST: usize = 8 * 1024;

/// Starts the supervisor: it (re)binds when the setting or port changes and stops when off.
pub fn start(core: Arc<Core>) {
    tauri::async_runtime::spawn(async move {
        let mut bound: Option<(u16, TcpListener)> = None;
        loop {
            let (enabled, port) = {
                let s = core.settings.read();
                (s.remote.enabled, s.remote.port)
            };
            if !enabled {
                bound = None;
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            if bound.as_ref().is_none_or(|(p, _)| *p != port) {
                match TcpListener::bind(("0.0.0.0", port)).await {
                    Ok(l) => {
                        tracing::info!("lan dashboard on port {port}");
                        bound = Some((port, l));
                    }
                    Err(e) => {
                        tracing::warn!("lan dashboard port {port}: {e}");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                }
            }
            let Some((_, listener)) = &bound else { continue };
            // Wake up regularly to notice setting changes.
            if let Ok(Ok((stream, _))) = tokio::time::timeout(Duration::from_secs(1), listener.accept()).await {
                let c = core.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = serve(stream, c).await {
                        tracing::debug!("lan dashboard client: {e}");
                    }
                });
            }
        }
    });
}

/// Path and PIN of a request line "GET /live?pin=1234 HTTP/1.1".
fn parse_request(head: &str) -> Option<(&str, Option<&str>)> {
    let target = head.lines().next()?.strip_prefix("GET ")?.split(' ').next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let pin = query.split('&').find_map(|kv| kv.strip_prefix("pin="));
    Some((path, pin))
}

async fn serve(mut stream: TcpStream, core: Arc<Core>) -> anyhow::Result<()> {
    let mut buf = vec![0u8; MAX_REQUEST];
    let mut n = 0;
    while n < buf.len() {
        let read = stream.read(&mut buf[n..]).await?;
        if read == 0 {
            break;
        }
        n += read;
        if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf[..n]).into_owned();
    let Some((path, pin)) = parse_request(&head) else {
        stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n").await?;
        return Ok(());
    };
    let wanted = core.settings.read().remote.pin.clone();
    let allowed = wanted.is_empty() || pin == Some(wanted.as_str());
    match path {
        "/" | "/index.html" => {
            let body = PAGE.as_bytes();
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\r\n", body.len());
            stream.write_all(head.as_bytes()).await?;
            stream.write_all(body).await?;
        }
        "/live" if !allowed => {
            stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n").await?;
        }
        "/live" => {
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n").await?;
            let mut k = 0u64;
            loop {
                if !core.settings.read().remote.enabled {
                    break;
                }
                let frame = crate::live::build_live(&core, k % HEAVY_EVERY == 0);
                let json = serde_json::to_string(&frame)?;
                stream.write_all(format!("data: {json}\n\n").as_bytes()).await?;
                k += 1;
                tokio::time::sleep(Duration::from_millis(1000 / RATE_HZ)).await;
            }
        }
        _ => {
            stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await?;
        }
    }
    Ok(())
}

/// QR code of a text as a square matrix of dark modules (row by row).
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct QrMatrix {
    pub width: u32,
    pub dark: Vec<bool>,
}

pub fn qr(text: &str) -> Option<QrMatrix> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let dark = code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect();
    Some(QrMatrix { width: code.width() as u32, dark })
}

/// Addresses to type on the phone: this PC's network address(es) with the port.
pub fn urls(port: u16) -> Vec<String> {
    // Connecting a UDP socket only picks the outgoing interface; nothing is sent.
    let ip = std::net::UdpSocket::bind("0.0.0.0:0").and_then(|s| {
        s.connect("192.0.2.1:9")?;
        s.local_addr()
    });
    match ip {
        Ok(a) if !a.ip().is_unspecified() => vec![format!("http://{}:{port}", a.ip())],
        _ => vec![format!("http://localhost:{port}")],
    }
}

#[cfg(test)]
mod tests {
    use super::parse_request;

    #[test]
    fn qr_for_an_address() {
        let q = super::qr("http://192.168.1.20:8765").unwrap();
        assert_eq!(q.dark.len(), (q.width * q.width) as usize);
        assert!(q.width >= 21 && q.dark.iter().any(|d| *d));
    }

    #[test]
    fn request_line() {
        assert_eq!(parse_request("GET /live?pin=1234 HTTP/1.1\r\nHost: x\r\n\r\n"), Some(("/live", Some("1234"))));
        assert_eq!(parse_request("GET / HTTP/1.1\r\n\r\n"), Some(("/", None)));
        assert_eq!(parse_request("POST / HTTP/1.1\r\n\r\n"), None);
    }
}
