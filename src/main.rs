fn main() {
    if let Err(e) = mac_cleaner::run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
