fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("start") | None => {
            if let Err(e) = bls::server::run() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some("--version") => {
            println!("bls {}", env!("CARGO_PKG_VERSION"));
        }
        Some("--help") => {
            println!("Usage: bls [start|--version|--help]");
        }
        _ => {
            eprintln!("Unknown command");
            std::process::exit(1);
        }
    }
}
