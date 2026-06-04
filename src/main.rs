const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let log_level = args
        .iter()
        .find_map(|a| a.strip_prefix("--log-level=").map(str::to_string))
        .or_else(|| {
            args.iter()
                .position(|a| a == "--log-level")
                .and_then(|i| args.get(i + 1).cloned())
        })
        .unwrap_or_else(|| "error".to_string());

    if !VALID_LOG_LEVELS.contains(&log_level.as_str()) {
        eprintln!(
            "Invalid log level '{}'. Valid levels: {}",
            log_level,
            VALID_LOG_LEVELS.join(", ")
        );
        std::process::exit(1);
    }

    let env = env_logger::Env::default().default_filter_or(format!("bashls={log_level}"));
    env_logger::Builder::from_env(env).init();

    let mut skip_next = false;
    let cmd = args.iter().skip(1).find(|a| {
        if skip_next {
            skip_next = false;
            return false;
        }
        if *a == "--log-level" {
            skip_next = true;
            return false;
        }
        if a.starts_with("--log-level=") {
            return false;
        }
        true
    });

    match cmd.map(String::as_str) {
        Some("start") | None => {
            if let Err(e) = bashls::server::run() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some("--version") | Some("-v") => {
            println!("bashls {}", env!("CARGO_PKG_VERSION"));
        }
        Some("--help") | Some("-h") => {
            println!(
                "Usage: bashls [start|--version|--help] [--log-level error|warn|info|debug|trace]"
            );
        }
        _ => {
            eprintln!("Unknown command");
            std::process::exit(1);
        }
    }
}
