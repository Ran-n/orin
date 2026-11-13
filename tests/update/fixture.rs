// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:38.812134
//+ Revised: 	2026/11/12 22:43:38.812134
// -----------------------------------------------------------------------------

//! A tiny HTTP/1.1 server on `127.0.0.1` for update tests: fixed replies per path, request
//! counts, error statuses, and connections that never answer.
#![allow(dead_code)]

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct Reply {
    status: u16,
    body: Vec<u8>,
    /// A `Content-Length` other than the body's real size.
    declared: Option<u64>,
    /// Accept the request and never answer.
    hang: bool,
    /// Pause between 64 KiB chunks of the body.
    pace: Option<Duration>,
}

impl Reply {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            declared: None,
            hang: false,
            pace: None,
        }
    }

    pub fn status(status: u16) -> Self {
        Self {
            status,
            body: br#"{"message":"refused"}"#.to_vec(),
            declared: None,
            hang: false,
            pace: None,
        }
    }

    pub fn hang() -> Self {
        Self {
            hang: true,
            ..Self::status(200)
        }
    }

    /// Send the body slowly, so a test can act in the middle of a download.
    pub fn paced(mut self, pause: Duration) -> Self {
        self.pace = Some(pause);
        self
    }

    pub fn declaring_length(mut self, length: u64) -> Self {
        self.declared = Some(length);
        self
    }
}

pub struct FixtureServer {
    address: String,
    routes: Arc<Mutex<HashMap<String, Reply>>>,
    counts: Arc<Mutex<HashMap<String, Requests>>>,
    stop: Arc<AtomicBool>,
}

/// How many requests reached a path, and the last one's headers.
type Requests = (usize, Vec<(String, String)>);

impl FixtureServer {
    pub fn start(routes: Vec<(&str, Reply)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the fixture server");
        listener
            .set_nonblocking(true)
            .expect("non-blocking listener");
        let address = format!("http://{}", listener.local_addr().unwrap());
        let routes = Arc::new(Mutex::new(
            routes
                .into_iter()
                .map(|(path, reply)| (path.to_owned(), reply))
                .collect::<HashMap<_, _>>(),
        ));
        let counts = Arc::new(Mutex::new(HashMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (served, counted, stopping) = (routes.clone(), counts.clone(), stop.clone());
        thread::Builder::new()
            .name("update-fixture".into())
            .spawn(move || {
                while !stopping.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let (served, counted, stopping) =
                                (served.clone(), counted.clone(), stopping.clone());
                            thread::spawn(move || serve(stream, &served, &counted, &stopping));
                        }
                        Err(_) => thread::sleep(Duration::from_millis(5)),
                    }
                }
            })
            .expect("start the fixture server");
        Self {
            address,
            routes,
            counts,
            stop,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.address)
    }

    /// Replace or add the reply for `path`.
    pub fn route(&self, path: &str, reply: Reply) {
        self.routes.lock().unwrap().insert(path.to_owned(), reply);
    }

    /// How many requests reached `path`.
    pub fn requests(&self, path: &str) -> usize {
        self.counts
            .lock()
            .unwrap()
            .get(path)
            .map_or(0, |requests| requests.0)
    }

    /// The headers of the last request to `path`, names lower-cased.
    pub fn headers(&self, path: &str) -> Vec<(String, String)> {
        self.counts
            .lock()
            .unwrap()
            .get(path)
            .map(|requests| requests.1.clone())
            .unwrap_or_default()
    }

    /// Requests to every path.
    pub fn total_requests(&self) -> usize {
        self.counts
            .lock()
            .unwrap()
            .values()
            .map(|requests| requests.0)
            .sum()
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn serve(
    stream: TcpStream,
    routes: &Mutex<HashMap<String, Reply>>,
    counts: &Mutex<HashMap<String, Requests>>,
    stop: &AtomicBool,
) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => return,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {
                if let Some((name, value)) = header.split_once(':') {
                    headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
                }
            }
        }
    }
    {
        let mut log = counts.lock().unwrap();
        let entry = log.entry(path.clone()).or_default();
        entry.0 += 1;
        entry.1 = headers;
    }
    let reply = routes
        .lock()
        .unwrap()
        .get(&path)
        .cloned()
        .unwrap_or_else(|| Reply::status(404));
    if reply.hang {
        while !stop.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(20));
        }
        return;
    }
    let mut stream = stream;
    let length = reply.declared.unwrap_or(reply.body.len() as u64);
    let head = format!(
        "HTTP/1.1 {} Fixture\r\nContent-Length: {length}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        reply.status
    );
    // A client may hang up early (a refused size, a cancel): that is the point of the test.
    let _ = stream.write_all(head.as_bytes());
    match reply.pace {
        None => {
            let _ = stream.write_all(&reply.body);
        }
        Some(pause) => {
            for chunk in reply.body.chunks(64 * 1024) {
                if stop.load(Ordering::Acquire) || stream.write_all(chunk).is_err() {
                    return;
                }
                thread::sleep(pause);
            }
        }
    }
    let _ = stream.flush();
}
