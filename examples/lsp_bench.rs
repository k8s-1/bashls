// LSP integration benchmark: bashls vs bash-language-server
//
// Sends real LSP protocol messages to each server over stdin/stdout and measures
// startup latency, request latency (completion + hover), and RSS memory.
//
// Corpus: .sh files from oh-my-bash (https://github.com/ohmybash/oh-my-bash).
// Tested against bash-language-server 5.6.0 (https://github.com/bash-lsp/bash-language-server).
//
// Usage:
//   git clone https://github.com/ohmybash/oh-my-bash /tmp/oh-my-bash
//   cargo run --example lsp_bench --release
//
// Environment variables:
//   BASHLS_BIN     path to bashls binary         (default: ./target/release/bashls)
//   BASH_LS_BIN    path to bash-language-server  (default: bash-language-server)
//   CORPUS_DIR     root of the .sh corpus        (default: /tmp/oh-my-bash)
//   CORPUS_FILES   max files to use              (default: 50)

use std::{
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use walkdir::WalkDir;

struct BenchResult {
    startup_ms: f64,
    latencies_ms: Vec<f64>, // sorted ascending
    rss_kb: u64,
}

// ── LSP framing ───────────────────────────────────────────────────────────────

fn lsp_encode(value: &Value) -> Vec<u8> {
    let body = serde_json::to_string(value).unwrap();
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

fn reader_thread(stdout: std::process::ChildStdout, tx: mpsc::Sender<(u64, Instant)>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => return,
                _ => {}
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(v) = trimmed.strip_prefix("Content-Length:") {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }
        if content_length == 0 {
            continue;
        }
        let mut body = vec![0u8; content_length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        if let Ok(msg) = serde_json::from_slice::<Value>(&body) {
            if let Some(id) = msg["id"].as_u64() {
                let _ = tx.send((id, Instant::now()));
            }
        }
    }
}

// ── LSP session ───────────────────────────────────────────────────────────────

struct LspSession {
    child: Child,
    stdin: ChildStdin,
    rx: mpsc::Receiver<(u64, Instant)>,
    next_id: u64,
}

impl LspSession {
    fn spawn(program: &str, args: &[&str]) -> Self {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {program}: {e}"));

        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || reader_thread(stdout, tx));

        LspSession { child, stdin, rx, next_id: 1 }
    }

    fn notify(&mut self, msg: Value) {
        self.stdin.write_all(&lsp_encode(&msg)).unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, mut msg: Value) -> Duration {
        let id = self.next_id;
        self.next_id += 1;
        msg["id"] = json!(id);
        let t_sent = Instant::now();
        self.stdin.write_all(&lsp_encode(&msg)).unwrap();
        self.stdin.flush().unwrap();
        self.wait_for(id, Duration::from_secs(30)).duration_since(t_sent)
    }

    fn wait_for(&self, id: u64, timeout: Duration) -> Instant {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(remaining) {
                Ok((recv_id, t)) if recv_id == id => return t,
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    panic!("timeout waiting for LSP response to request {id}")
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("reader thread exited before response to request {id}")
                }
            }
        }
    }

    fn rss_kb(&self) -> u64 {
        fs::read_to_string(format!("/proc/{}/status", self.child.id()))
            .unwrap_or_default()
            .lines()
            .find_map(|line| {
                let rest = line.strip_prefix("VmRSS:")?;
                rest.split_whitespace().next()?.parse().ok()
            })
            .unwrap_or(0)
    }
}

impl Drop for LspSession {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

// ── Benchmark run ─────────────────────────────────────────────────────────────

fn run_bench(program: &str, args: &[&str], label: &str, files: &[(String, String)]) -> BenchResult {
    println!("\n[{label}]");
    let mut session = LspSession::spawn(program, args);

    let startup_ms = session
        .request(json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": "file:///tmp",
                "capabilities": {
                    "textDocument": {
                        "completion": {"completionItem": {"snippetSupport": true}},
                        "hover": {}
                    }
                }
            }
        }))
        .as_secs_f64()
        * 1000.0;
    println!("  startup:     {startup_ms:.1} ms");

    session.notify(json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));

    let file_uris: Vec<String> = files
        .iter()
        .map(|(path, text)| {
            let uri = format!("file://{path}");
            session.notify(json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {"uri": uri, "languageId": "sh", "version": 1, "text": text}
                }
            }));
            uri
        })
        .collect();

    // LSP messages are ordered; the response to this request arriving means
    // all prior didOpen notifications have been processed. Avoids an arbitrary sleep.
    let last_uri = file_uris.last().unwrap();
    session.request(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/completion",
        "params": {"textDocument": {"uri": last_uri}, "position": {"line": 0, "character": 0}}
    }));

    for ((_, text), uri) in files.iter().zip(&file_uris) {
        session.notify(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": uri, "version": 2},
                "contentChanges": [{"text": format!("{text}\n# edit\n")}]
            }
        }));
    }
    session.request(json!({
        "jsonrpc": "2.0",
        "method": "textDocument/completion",
        "params": {"textDocument": {"uri": last_uri}, "position": {"line": 0, "character": 0}}
    }));

    // Sequential requests: one in-flight at a time so that queuing delay does
    // not inflate individual latency samples.
    let methods = ["textDocument/completion", "textDocument/hover"];
    let mut latencies_ms = Vec::new();
    for uri in &file_uris {
        for line in 0u32..25 {
            for method in methods {
                let d = session.request(json!({
                    "jsonrpc": "2.0",
                    "method": method,
                    "params": {
                        "textDocument": {"uri": uri},
                        "position": {"line": line, "character": 4}
                    }
                }));
                latencies_ms.push(d.as_secs_f64() * 1000.0);
            }
        }
    }
    latencies_ms.sort_by(f64::total_cmp);

    let rss_kb = session.rss_kb();
    drop(session);

    if !latencies_ms.is_empty() {
        let avg = latencies_ms.iter().sum::<f64>() / latencies_ms.len() as f64;
        let p95 = percentile(&latencies_ms, 95);
        let p99 = percentile(&latencies_ms, 99);
        println!("  requests:    {}", latencies_ms.len());
        println!("  latency avg: {avg:.1} ms");
        println!("  latency p95: {p95:.1} ms");
        println!("  latency p99: {p99:.1} ms");
    }
    println!("  RSS:         {rss_kb} kB  ({:.1} MB)", rss_kb as f64 / 1024.0);

    BenchResult { startup_ms, latencies_ms, rss_kb }
}

// ── Stats ─────────────────────────────────────────────────────────────────────

// Nearest-rank percentile on a sorted slice (p in 0..=100).
fn percentile(sorted: &[f64], p: usize) -> f64 {
    assert!(!sorted.is_empty() && p <= 100);
    // ceil(p/100 * n) - 1, expressed with integer arithmetic to avoid fp rounding.
    let idx = ((p * sorted.len()).saturating_sub(1) / 100).min(sorted.len() - 1);
    sorted[idx]
}

fn ratio_str(a: f64, b: f64) -> String {
    if a > 0.0 { format!("{:.1}x", b / a) } else { "n/a".into() }
}

fn print_summary(file_count: usize, bashls: &BenchResult, bash_ls: &BenchResult) {
    // latencies_ms is pre-sorted in run_bench
    let lat1 = &bashls.latencies_ms;
    let lat2 = &bash_ls.latencies_ms;

    let avg = |v: &[f64]| -> f64 {
        if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
    };
    let safe_pct = |v: &[f64], p| -> f64 {
        if v.is_empty() { 0.0 } else { percentile(v, p) }
    };

    println!(
        "\n--- summary ({file_count} files, {} requests each) ---",
        lat1.len()
    );
    println!(
        "{:<22} {:>10} {:>10} {:>16}",
        "metric", "bashls", "bash-ls", "ratio (b/a)"
    );

    let row = |label: &str, a: f64, b: f64| {
        println!("{:<22} {:>10.1} {:>10.1} {:>16}", label, a, b, ratio_str(a, b));
    };
    row("startup (ms)", bashls.startup_ms, bash_ls.startup_ms);
    row("latency avg (ms)", avg(lat1), avg(lat2));
    row("latency p95 (ms)", safe_pct(lat1, 95), safe_pct(lat2, 95));
    row(
        "RSS (MB)",
        bashls.rss_kb as f64 / 1024.0,
        bash_ls.rss_kb as f64 / 1024.0,
    );
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let bashls_bin = env::var("BASHLS_BIN").unwrap_or_else(|_| "./target/release/bashls".into());
    let bash_ls_bin = env::var("BASH_LS_BIN").unwrap_or_else(|_| "bash-language-server".into());
    let corpus_dir = env::var("CORPUS_DIR").unwrap_or_else(|_| "/tmp/oh-my-bash".into());
    let corpus_max: usize = env::var("CORPUS_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let files: Vec<(String, String)> = WalkDir::new(&corpus_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "sh"))
        .take(corpus_max)
        .filter_map(|e| {
            let path = e.path().to_string_lossy().into_owned();
            let text = fs::read_to_string(e.path()).ok()?;
            Some((path, text))
        })
        .collect();

    if files.is_empty() {
        eprintln!("No .sh files found in CORPUS_DIR={corpus_dir:?}.");
        eprintln!("Clone https://github.com/ohmybash/oh-my-bash there first.");
        std::process::exit(1);
    }

    let r1 = run_bench(&bashls_bin, &[], "bashls", &files);
    let r2 = run_bench(&bash_ls_bin, &["start"], "bash-language-server", &files);

    print_summary(files.len(), &r1, &r2);
}
