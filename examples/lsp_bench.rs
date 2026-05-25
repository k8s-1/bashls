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
//   CORPUS_DIR     root of the .sh corpus         (default: /tmp/oh-my-bash)
//   CORPUS_FILES   max files to use               (default: 50)

use std::{
    collections::HashMap,
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use walkdir::WalkDir;

struct BenchResult {
    startup_ms: f64,
    latencies: Vec<f64>,
    rss_kb: u64,
}

fn lsp_encode(obj: &Value) -> Vec<u8> {
    let body = serde_json::to_string(obj).unwrap();
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

fn send(stdin: &mut std::process::ChildStdin, obj: Value) {
    stdin.write_all(&lsp_encode(&obj)).ok();
    stdin.flush().ok();
}

fn read_loop(stdout: std::process::ChildStdout, responses: Arc<Mutex<HashMap<u64, Instant>>>) {
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
                responses.lock().unwrap().insert(id, Instant::now());
            }
        }
    }
}

fn rss_kb(pid: u32) -> u64 {
    let content = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn wait_for(responses: &Arc<Mutex<HashMap<u64, Instant>>>, id: u64, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if responses.lock().unwrap().contains_key(&id) {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn run_bench(program: &str, args: &[&str], label: &str, files: &[(String, String)]) -> BenchResult {
    println!("\n[{label}]");

    let responses: Arc<Mutex<HashMap<u64, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {program}: {e}"));

    let stdout = child.stdout.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();

    let resp_clone = Arc::clone(&responses);
    thread::spawn(move || read_loop(stdout, resp_clone));

    let t0 = Instant::now();
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
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
        }),
    );
    wait_for(&responses, 1, Duration::from_secs(10));
    let startup_ms = responses
        .lock()
        .unwrap()
        .get(&1)
        .map(|t| t.duration_since(t0).as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    println!("  startup:     {startup_ms:.1} ms");

    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );

    let file_uris: Vec<String> = files
        .iter()
        .map(|(path, text)| {
            let uri = format!("file://{path}");
            send(
                &mut stdin,
                json!({
                    "jsonrpc": "2.0", "method": "textDocument/didOpen",
                    "params": {
                        "textDocument": {"uri": uri, "languageId": "sh", "version": 1, "text": text}
                    }
                }),
            );
            uri
        })
        .collect();

    // wait for background analysis to settle before measuring
    thread::sleep(Duration::from_secs(1));

    for ((_, text), uri) in files.iter().zip(file_uris.iter()) {
        send(
            &mut stdin,
            json!({
                "jsonrpc": "2.0", "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri, "version": 2},
                    "contentChanges": [{"text": format!("{text}\n# edit\n")}]
                }
            }),
        );
    }

    thread::sleep(Duration::from_millis(500));

    let mut req_id: u64 = 2;
    let mut send_times: HashMap<u64, Instant> = HashMap::new();
    for uri in &file_uris {
        for line in 0u32..25 {
            for method in ["textDocument/completion", "textDocument/hover"] {
                send_times.insert(req_id, Instant::now());
                send(
                    &mut stdin,
                    json!({
                        "jsonrpc": "2.0", "id": req_id, "method": method,
                        "params": {
                            "textDocument": {"uri": uri},
                            "position": {"line": line, "character": 4}
                        }
                    }),
                );
                req_id += 1;
            }
        }
    }

    let total_requests = req_id - 2;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let n = responses.lock().unwrap().len();
        // +1 because the initialize response (id 1) is also in the map
        if n >= total_requests as usize + 1 {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let received = responses.lock().unwrap().clone();
    let latencies: Vec<f64> = send_times
        .iter()
        .filter_map(|(id, t_sent)| {
            received
                .get(id)
                .map(|t_recv| t_recv.duration_since(*t_sent).as_secs_f64() * 1000.0)
        })
        .collect();

    drop(stdin);
    let rss = rss_kb(child.id());
    child.kill().ok();
    child.wait().ok();

    if !latencies.is_empty() {
        let mut s = latencies.clone();
        s.sort_by(f64::total_cmp);
        let avg = s.iter().sum::<f64>() / s.len() as f64;
        let p95 = s[s.len() * 95 / 100];
        let p99 = s[s.len() * 99 / 100];
        println!(
            "  requests:    {}/{} answered",
            latencies.len(),
            send_times.len()
        );
        println!("  latency avg: {avg:.1} ms");
        println!("  latency p95: {p95:.1} ms");
        println!("  latency p99: {p99:.1} ms");
    }
    println!("  RSS:         {rss} kB  ({:.1} MB)", rss as f64 / 1024.0);

    BenchResult {
        startup_ms,
        latencies,
        rss_kb: rss,
    }
}

fn percentile(sorted: &[f64], p: usize) -> f64 {
    sorted[sorted.len() * p / 100]
}

fn ratio(a: f64, b: f64) -> String {
    if a > 0.0 {
        format!("{:.1}x", b / a)
    } else {
        "n/a".into()
    }
}

fn print_summary(file_count: usize, r1: &BenchResult, r2: &BenchResult) {
    let mut lat1 = r1.latencies.clone();
    let mut lat2 = r2.latencies.clone();
    lat1.sort_by(f64::total_cmp);
    lat2.sort_by(f64::total_cmp);

    let avg = |v: &[f64]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };

    let avg1 = avg(&lat1);
    let avg2 = avg(&lat2);
    let p95_1 = if lat1.is_empty() {
        0.0
    } else {
        percentile(&lat1, 95)
    };
    let p95_2 = if lat2.is_empty() {
        0.0
    } else {
        percentile(&lat2, 95)
    };

    let total = lat1.len() + lat2.len();
    println!("\n--- summary ({file_count} files, {total} total requests) ---");
    println!(
        "{:<22} {:>10} {:>10} {:>8}",
        "metric", "bashls", "bash-ls", "ratio"
    );
    println!(
        "{:<22} {:>10.1} {:>10.1} {:>8}",
        "startup (ms)",
        r1.startup_ms,
        r2.startup_ms,
        ratio(r1.startup_ms, r2.startup_ms)
    );
    println!(
        "{:<22} {:>10.1} {:>10.1} {:>8}",
        "latency avg (ms)",
        avg1,
        avg2,
        ratio(avg1, avg2)
    );
    println!(
        "{:<22} {:>10.1} {:>10.1} {:>8}",
        "latency p95 (ms)",
        p95_1,
        p95_2,
        ratio(p95_1, p95_2)
    );
    println!(
        "{:<22} {:>10.1} {:>10.1} {:>8}",
        "RSS (MB)",
        r1.rss_kb as f64 / 1024.0,
        r2.rss_kb as f64 / 1024.0,
        ratio(r1.rss_kb as f64, r2.rss_kb as f64)
    );
}

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
