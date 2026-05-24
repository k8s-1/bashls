fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
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
            println!("Usage: bashls [start|--version|--help]");
        }
        _ => {
            eprintln!("Unknown command");
            std::process::exit(1);
        }
    }
}
