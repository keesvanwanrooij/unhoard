//! The connection to Ollama for the AI chat (F8): a small HTTP/1.1 client that can reach
//! `127.0.0.1:11434` and nothing else (S8, T-8.3).
//!
//! There is no host or port parameter anywhere: the address is a constant, and the one
//! function that opens a socket refuses any other address. Requests are fixed paths with a
//! JSON body; replies are read with a size limit, plain or chunked. Ollama streams chat
//! replies as one JSON object per line.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Ollama's default local address.
pub const OLLAMA: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 11434);
/// The largest reply read (model lists and non-streamed replies).
const MAX_REPLY: usize = 4 << 20;
/// The longest line of a streamed reply.
const MAX_LINE: usize = 1 << 20;
/// How long a read may wait; a model can think for a while before the first token.
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// Why talking to Ollama failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatError {
    /// Nothing listens on 127.0.0.1:11434: Ollama is not installed or not running.
    NotRunning,
    /// Ollama answered with an error status (for example an unknown model).
    Refused,
    /// The reply could not be read or was too large.
    BadReply,
    /// The address is not Ollama's local one; never connected.
    NotAllowed,
    /// The chat is switched off in Settings; nothing was opened.
    Off,
}

/// Opens a connection to `addr`, which must be Ollama's local address.
fn connect(addr: SocketAddr) -> Result<TcpStream, ChatError> {
    if addr != SocketAddr::V4(OLLAMA) {
        return Err(ChatError::NotAllowed);
    }
    let s = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map_err(|_| ChatError::NotRunning)?;
    let _ = s.set_read_timeout(Some(READ_TIMEOUT));
    let _ = s.set_nodelay(true);
    Ok(s)
}

/// Sends one request and returns a reader positioned at the body, plus whether the body is
/// chunked.
fn request(
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(BufReader<TcpStream>, bool), ChatError> {
    let mut s = connect(SocketAddr::V4(OLLAMA))?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes())
        .and_then(|()| s.write_all(body))
        .map_err(|_| ChatError::NotRunning)?;
    let mut r = BufReader::new(s);
    let mut status = String::new();
    r.read_line(&mut status).map_err(|_| ChatError::BadReply)?;
    let code: u16 = status
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or(ChatError::BadReply)?;
    let mut chunked = false;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).map_err(|_| ChatError::BadReply)? == 0 {
            return Err(ChatError::BadReply);
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case("transfer-encoding")
            && v.trim().eq_ignore_ascii_case("chunked")
        {
            chunked = true;
        }
    }
    if !(200..300).contains(&code) {
        return Err(ChatError::Refused);
    }
    Ok((r, chunked))
}

/// Reads a whole body, plain or chunked, at most `max` bytes.
fn read_body(mut r: BufReader<TcpStream>, chunked: bool, max: usize) -> Result<Vec<u8>, ChatError> {
    let mut out = vec![];
    if !chunked {
        r.take(max as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|_| ChatError::BadReply)?;
        return if out.len() > max {
            Err(ChatError::BadReply)
        } else {
            Ok(out)
        };
    }
    loop {
        let mut size = String::new();
        r.read_line(&mut size).map_err(|_| ChatError::BadReply)?;
        let n = usize::from_str_radix(size.trim().split(';').next().unwrap_or(""), 16)
            .map_err(|_| ChatError::BadReply)?;
        if n == 0 {
            return Ok(out);
        }
        if out.len() + n > max {
            return Err(ChatError::BadReply);
        }
        let start = out.len();
        out.resize(start + n, 0);
        r.read_exact(&mut out[start..])
            .map_err(|_| ChatError::BadReply)?;
        let mut crlf = [0u8; 2];
        r.read_exact(&mut crlf).map_err(|_| ChatError::BadReply)?;
    }
}

/// `GET /api/tags`: the installed models, as Ollama's JSON.
pub fn tags() -> Result<String, ChatError> {
    let (r, chunked) = request("GET", "/api/tags", b"")?;
    let body = read_body(r, chunked, MAX_REPLY)?;
    String::from_utf8(body).map_err(|_| ChatError::BadReply)
}

/// `POST /api/chat` with a streamed reply: `on_line` gets each JSON line and returns false
/// to stop reading (the user pressed stop).
pub fn chat_stream(body: &[u8], on_line: &mut dyn FnMut(&str) -> bool) -> Result<(), ChatError> {
    stream("/api/chat", body, on_line)
}

/// `POST /api/pull`: Ollama downloads `model` from its library and streams its progress as
/// JSON lines (`status`, `total`, `completed`). `on_line` returns false to stop listening;
/// Ollama then finishes or resumes the download on its own.
pub fn pull_stream(model: &str, on_line: &mut dyn FnMut(&str) -> bool) -> Result<(), ChatError> {
    let body = serde_json::to_vec(&serde_json::json!({"model": model, "stream": true}))
        .map_err(|_| ChatError::BadReply)?;
    stream("/api/pull", &body, on_line)
}

/// A request whose reply is one JSON object per line.
fn stream(path: &str, body: &[u8], on_line: &mut dyn FnMut(&str) -> bool) -> Result<(), ChatError> {
    let (mut r, chunked) = request("POST", path, body)?;
    let mut pending: Vec<u8> = vec![];
    let mut emit = |bytes: &[u8], pending: &mut Vec<u8>| -> Result<bool, ChatError> {
        pending.extend_from_slice(bytes);
        while let Some(i) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=i).collect();
            let text = String::from_utf8_lossy(&line);
            let text = text.trim();
            if !text.is_empty() && !on_line(text) {
                return Ok(false);
            }
        }
        if pending.len() > MAX_LINE {
            return Err(ChatError::BadReply);
        }
        Ok(true)
    };
    if !chunked {
        let mut buf = [0u8; 8192];
        loop {
            let n = r.read(&mut buf).map_err(|_| ChatError::BadReply)?;
            if n == 0 || !emit(&buf[..n], &mut pending)? {
                break;
            }
        }
    } else {
        loop {
            let mut size = String::new();
            r.read_line(&mut size).map_err(|_| ChatError::BadReply)?;
            let n = usize::from_str_radix(size.trim().split(';').next().unwrap_or(""), 16)
                .map_err(|_| ChatError::BadReply)?;
            if n == 0 {
                break;
            }
            if n > MAX_LINE {
                return Err(ChatError::BadReply);
            }
            let mut chunk = vec![0u8; n];
            r.read_exact(&mut chunk).map_err(|_| ChatError::BadReply)?;
            let mut crlf = [0u8; 2];
            r.read_exact(&mut crlf).map_err(|_| ChatError::BadReply)?;
            if !emit(&chunk, &mut pending)? {
                break;
            }
        }
    }
    if !pending.is_empty() {
        let text = String::from_utf8_lossy(&pending).trim().to_string();
        if !text.is_empty() {
            on_line(&text);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f8_t8_3_only_ollamas_local_address_is_ever_connected() {
        for other in [
            "10.0.0.5:11434",
            "127.0.0.1:80",
            "127.0.0.2:11434",
            "[::1]:11434",
            "93.184.216.34:443",
        ] {
            let addr: SocketAddr = other.parse().unwrap_or(SocketAddr::V4(OLLAMA));
            if addr == SocketAddr::V4(OLLAMA) {
                continue;
            }
            assert_eq!(connect(addr).err(), Some(ChatError::NotAllowed), "{other}");
        }
    }
}
