//! LSP-style Content-Length framing for JSON-RPC messages over a byte
//! stream (stdio, Unix-domain socket, pipe).
//!
//! Each message is:
//!
//! ```text
//! Content-Length: <N>\r\n
//! \r\n
//! <N bytes of UTF-8 JSON>
//! ```
//!
//! This matches the LSP convention and is what tmuxwright adapters
//! will speak over stdio; TCP/UDS transports can reuse the same framing.

use std::io::{self, BufRead, Write};

/// Write one framed message to `w`. The body must already be a
/// serialized JSON string.
///
/// # Errors
/// Propagates any I/O error from the underlying writer.
pub fn write_message<W: Write>(w: &mut W, body: &str) -> io::Result<()> {
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(body.as_bytes())?;
    w.flush()
}

/// Parse-error flavor distinct from I/O.
#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    MissingContentLength,
    MalformedHeader(String),
    UnexpectedEof,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::MissingContentLength => write!(f, "missing Content-Length header"),
            Self::MalformedHeader(h) => write!(f, "malformed header: {h:?}"),
            Self::UnexpectedEof => write!(f, "unexpected eof"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Read one framed message from `r`, returning its JSON body as
/// UTF-8. Returns `Ok(None)` at clean EOF before any bytes.
///
/// # Errors
/// Returns [`FrameError`] on malformed framing, truncated body, or I/O failure.
pub fn read_message<R: BufRead>(r: &mut R) -> Result<Option<String>, FrameError> {
    let mut content_length: Option<usize> = None;
    let mut header_line = String::new();
    let mut saw_any = false;

    loop {
        header_line.clear();
        let n = r.read_line(&mut header_line)?;
        if n == 0 {
            if saw_any {
                return Err(FrameError::UnexpectedEof);
            }
            return Ok(None);
        }
        saw_any = true;
        if header_line == "\r\n" || header_line == "\n" {
            break; // end of headers
        }
        let line = header_line.trim_end_matches(['\r', '\n']);
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| FrameError::MalformedHeader(line.to_string()))?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            let v: usize = value
                .trim()
                .parse()
                .map_err(|_| FrameError::MalformedHeader(line.to_string()))?;
            content_length = Some(v);
        }
        // Ignore unknown headers (e.g. Content-Type).
    }

    let len = content_length.ok_or(FrameError::MissingContentLength)?;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    String::from_utf8(body)
        .map(Some)
        .map_err(|e| FrameError::MalformedHeader(format!("non-utf8 body: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_single_message() {
        let mut buf = Vec::new();
        write_message(&mut buf, r#"{"jsonrpc":"2.0","method":"ping","id":1}"#).unwrap();
        let mut r = Cursor::new(buf);
        let out = read_message(&mut r).unwrap().unwrap();
        assert!(out.contains(r#""method":"ping""#));
    }

    #[test]
    fn roundtrip_two_messages_in_sequence() {
        let mut buf = Vec::new();
        write_message(&mut buf, r#"{"a":1}"#).unwrap();
        write_message(&mut buf, r#"{"b":2}"#).unwrap();
        let mut r = Cursor::new(buf);
        assert_eq!(read_message(&mut r).unwrap().as_deref(), Some(r#"{"a":1}"#));
        assert_eq!(read_message(&mut r).unwrap().as_deref(), Some(r#"{"b":2}"#));
        assert!(read_message(&mut r).unwrap().is_none());
    }

    #[test]
    fn clean_eof_before_any_bytes_returns_none() {
        let mut r = Cursor::new(Vec::new());
        assert!(read_message(&mut r).unwrap().is_none());
    }

    #[test]
    fn truncated_body_surfaces_io_error() {
        let mut buf = Vec::new();
        write!(&mut buf, "Content-Length: 10\r\n\r\n").unwrap();
        buf.extend_from_slice(b"short");
        let mut r = Cursor::new(buf);
        let err = read_message(&mut r).unwrap_err();
        matches!(err, FrameError::Io(_))
            .then_some(())
            .expect("io err");
    }

    #[test]
    fn missing_content_length_errors() {
        let mut buf = Vec::new();
        write!(&mut buf, "X-Other: 1\r\n\r\n").unwrap();
        buf.extend_from_slice(b"body");
        let mut r = Cursor::new(buf);
        match read_message(&mut r).unwrap_err() {
            FrameError::MissingContentLength => {}
            other => panic!("wrong variant: {other}"),
        }
    }

    #[test]
    fn unknown_headers_are_ignored() {
        let mut buf = Vec::new();
        write!(
            &mut buf,
            "Content-Type: application/json\r\nContent-Length: 2\r\n\r\nhi"
        )
        .unwrap();
        let mut r = Cursor::new(buf);
        assert_eq!(read_message(&mut r).unwrap().as_deref(), Some("hi"));
    }

    #[test]
    fn header_case_insensitive() {
        let mut buf = Vec::new();
        write!(&mut buf, "content-length: 3\r\n\r\nabc").unwrap();
        let mut r = Cursor::new(buf);
        assert_eq!(read_message(&mut r).unwrap().as_deref(), Some("abc"));
    }
}
