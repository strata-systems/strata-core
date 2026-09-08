//! Shared helpers for cloud chat providers.
use crate::wire::ChatRequest;
use crate::InferenceError;

/// Rejects knobs no cloud provider can honor. GBNF `grammar` is a llama.cpp
/// constraint; cloud callers should use `response_format` instead.
pub(crate) fn reject_local_only(
    request: &ChatRequest,
    provider: &str,
) -> Result<(), InferenceError> {
    if request.grammar.is_some() {
        return Err(InferenceError::Provider(format!(
            "{provider}: GBNF `grammar` is local-only; use `response_format` for cloud models"
        )));
    }
    Ok(())
}

/// A stand-in for a provider's API: one loopback HTTP/1.1 server that answers
/// its first request with a canned status and body, and hands back what the
/// client sent. Lets a test drive the real request path — URL, headers, body,
/// status handling, parsing — without a network or a key.
#[cfg(test)]
pub(crate) mod test_server {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread::JoinHandle;

    pub(crate) struct CannedResponse {
        base_url: String,
        request: JoinHandle<String>,
    }

    impl CannedResponse {
        /// Start serving `status` + `body` on a fresh loopback port.
        pub(crate) fn serve(status: u16, body: &str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
            let base_url = format!(
                "http://{}",
                listener.local_addr().expect("the bound address")
            );
            let body = body.to_string();
            let request = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept the client");
                let request = read_request(&mut stream);
                let response = format!(
                    "HTTP/1.1 {status} Canned\r\n\
                     content-type: application/json\r\n\
                     content-length: {}\r\n\
                     connection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write the response");
                request
            });
            Self { base_url, request }
        }

        /// The URL to hand a provider as its API root.
        pub(crate) fn base_url(&self) -> &str {
            &self.base_url
        }

        /// The raw request the client sent — head and body — once it has.
        pub(crate) fn request(self) -> String {
            self.request.join().expect("the server thread")
        }
    }

    /// Read one HTTP/1.1 request: the head up to the blank line, then as many
    /// body bytes as `content-length` promises.
    fn read_request(stream: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 4096];
        let head_end = loop {
            let read = stream.read(&mut chunk).expect("read the request head");
            assert!(read > 0, "client closed before sending a request head");
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(blank) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                break blank + 4;
            }
        };
        let head = String::from_utf8_lossy(&bytes[..head_end]).into_owned();
        let content_length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("a numeric content-length"))
            })
            // A request without the header has no body to wait for.
            .unwrap_or(0);
        while bytes.len() < head_end + content_length {
            let read = stream.read(&mut chunk).expect("read the request body");
            assert!(read > 0, "client closed mid-body");
            bytes.extend_from_slice(&chunk[..read]);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }
}
