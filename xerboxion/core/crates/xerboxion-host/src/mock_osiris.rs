//! A tiny, deterministic **mock OSIRIS** HTTP server for the `osiris` demo + the
//! integration test.
//!
//! OSIRIS (José's *machine à veille*) is a Next.js app whose `GET /api/*` routes
//! return JSON of live world events. For a hermetic, reproducible demo we do NOT
//! depend on the live upstreams (USGS / ADS-B / DeepState / CISA): instead we
//! serve **the exact OSIRIS-shaped JSON fixtures** committed under
//! `ploxions/osiris-adapter/fixtures/` from a throwaway localhost server. The
//! `osiris-adapter` is then pointed at this server's base URL and does a REAL
//! brokered `plc_fetch` against it (over the loopback) — the SAME code path it
//! runs against a real OSIRIS, just with a deterministic upstream.
//!
//! The server is intentionally minimal: blocking `std::net`, no async runtime,
//! no extra deps. It binds `127.0.0.1:0` (an OS-chosen free port), serves a
//! fixed route table, replies `404` for anything else, and shuts down when the
//! returned [`MockOsiris`] is dropped.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// The committed OSIRIS-shaped fixtures, baked into the binary so the demo/test
/// need no files at runtime. Each is the real response shape of its route.
const F_QUAKES: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/earthquakes.json");
const F_FLIGHTS: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/flights.json");
const F_FRONT: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/frontlines.json");
const F_CYBER: &str = include_str!("../../../ploxions/osiris-adapter/fixtures/cyber-threats.json");

/// Map an OSIRIS request path to its fixture body (the adapter's route subset).
fn route_body(path: &str) -> Option<&'static str> {
    match path {
        "/api/earthquakes" => Some(F_QUAKES),
        "/api/flights" => Some(F_FLIGHTS),
        "/api/frontlines" => Some(F_FRONT),
        "/api/cyber-threats" => Some(F_CYBER),
        _ => None,
    }
}

/// A running mock OSIRIS server. Drop it to stop the server thread.
pub struct MockOsiris {
    base: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MockOsiris {
    /// Bind `127.0.0.1:0` and start serving the OSIRIS fixtures on a background
    /// thread. Returns the running server (its [`base_url`] is the adapter's
    /// `osiris.refresh {"base":..}` target).
    pub fn start() -> std::io::Result<MockOsiris> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let base = format!("http://{addr}");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = handle_conn(stream);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(MockOsiris {
            base,
            stop,
            handle: Some(handle),
        })
    }

    /// The base URL (e.g. `http://127.0.0.1:54321`) the adapter polls.
    pub fn base_url(&self) -> &str {
        &self.base
    }
}

impl Drop for MockOsiris {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Nudge the accept loop with a throwaway connection so it observes `stop`
        // promptly, then join.
        let _ = TcpStream::connect(self.base.trim_start_matches("http://"));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Read one HTTP request line, route it to a fixture, and write the response.
fn handle_conn(mut stream: TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_millis(500)))?;
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);
    // First line: "GET /api/earthquakes HTTP/1.1"
    let path = req
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");

    let (status, body) = match route_body(path) {
        Some(b) => ("200 OK", b),
        None => ("404 Not Found", "{\"error\":\"no such route\"}"),
    };

    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_serves_each_fixture_route_and_404s_unknown() {
        let mock = MockOsiris::start().unwrap();
        let base = mock.base_url().to_string();
        let get = |p: &str| {
            ureq::get(&format!("{base}{p}"))
                .call()
                .map(|mut r| (r.status().as_u16(), r.body_mut().read_to_string().unwrap_or_default()))
        };
        let (code, body) = get("/api/earthquakes").unwrap();
        assert_eq!(code, 200);
        assert!(body.contains("earthquakes"));
        let (code, body) = get("/api/cyber-threats").unwrap();
        assert_eq!(code, 200);
        assert!(body.contains("threats"));
        // Unknown route -> 404 (ureq surfaces it as a StatusCode error).
        let err = get("/api/nope").unwrap_err();
        match err {
            ureq::Error::StatusCode(c) => assert_eq!(c, 404),
            other => panic!("expected 404, got {other:?}"),
        }
    }
}
