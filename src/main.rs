const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

fn main() {
    let mut log_level = "error".to_string();
    let mut cmd: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(level) = arg.strip_prefix("--log-level=") {
            log_level = level.to_string();
        } else if arg == "--log-level" {
            if let Some(level) = args.next() {
                log_level = level;
            }
        } else if cmd.is_none() {
            cmd = Some(arg);
        }
    }

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

    match cmd.as_deref() {
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
