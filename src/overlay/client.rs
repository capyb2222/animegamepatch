use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 8088;
const POLL: Duration = Duration::from_millis(250);
const TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Clone, Default)]
pub struct Row {
    pub name: String,
    pub owner: String,
    pub element: u32,
    pub dealt: f64,
    pub taken: f64,
    pub max_hit: f64,
    pub hits: u64,
    pub dps: f64,
}

#[derive(Clone, Default)]
pub struct Hit {
    pub ago: f64,
    pub name: String,
    pub target: String,
    pub damage: f64,
    pub element: u32,
}

#[derive(Clone, Default)]
pub struct Snapshot {
    pub connected: bool,
    pub online: bool,
    pub active: bool,
    pub duration: f64,
    pub total: f64,
    pub total_taken: f64,
    pub dps: f64,
    pub rows: Vec<Row>,
    pub hits: Vec<Hit>,
}

static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);
static RESET: AtomicBool = AtomicBool::new(false);

pub fn snapshot() -> Snapshot {
    SNAPSHOT
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .unwrap_or_default()
}

pub fn request_reset() {
    RESET.store(true, Ordering::Relaxed);
}

fn request(method: &str, path: &str) -> Option<String> {
    let addr: SocketAddr = format!("{HOST}:{PORT}").parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;

    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {HOST}:{PORT}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;

    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw);

    let body = text.split_once("\r\n\r\n")?.1;
    let start = body.find('{')?;
    let end = body.rfind('}')?;
    (end >= start).then(|| body[start..=end].to_string())
}

fn str_of(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn parse(body: &str) -> Option<Snapshot> {
    let v: Value = serde_json::from_str(body).ok()?;

    let rows = v["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|r| Row {
                    name: str_of(r, "name"),
                    owner: str_of(r, "owner"),
                    element: r["element"].as_u64().unwrap_or(0) as u32,
                    dealt: r["dealt"].as_f64().unwrap_or(0.0),
                    taken: r["taken"].as_f64().unwrap_or(0.0),
                    max_hit: r["maxHit"].as_f64().unwrap_or(0.0),
                    hits: r["hits"].as_u64().unwrap_or(0),
                    dps: r["dps"].as_f64().unwrap_or(0.0),
                })
                .collect()
        })
        .unwrap_or_default();

    let hits = v["hits"]
        .as_array()
        .map(|hits| {
            hits.iter()
                .map(|h| Hit {
                    ago: h["ago"].as_f64().unwrap_or(0.0),
                    name: str_of(h, "name"),
                    target: str_of(h, "target"),
                    damage: h["damage"].as_f64().unwrap_or(0.0),
                    element: h["element"].as_u64().unwrap_or(0) as u32,
                })
                .collect()
        })
        .unwrap_or_default();

    Some(Snapshot {
        connected: true,
        online: v["online"].as_bool().unwrap_or(false),
        active: v["active"].as_bool().unwrap_or(false),
        duration: v["duration"].as_f64().unwrap_or(0.0),
        total: v["total"].as_f64().unwrap_or(0.0),
        total_taken: v["totalTaken"].as_f64().unwrap_or(0.0),
        dps: v["dps"].as_f64().unwrap_or(0.0),
        rows,
        hits,
    })
}

pub fn start() {
    std::thread::spawn(|| loop {
        if RESET.swap(false, Ordering::Relaxed) {
            let _ = request("POST", "/lunagc/damagelog/reset");
        }

        let next = request("GET", "/lunagc/damagelog")
            .and_then(|body| parse(&body))
            .unwrap_or_default();
        let connected = next.connected;
        if let Ok(mut slot) = SNAPSHOT.lock() {
            *slot = Some(next);
        }

        std::thread::sleep(if connected { POLL } else { POLL * 8 });
    });
}
