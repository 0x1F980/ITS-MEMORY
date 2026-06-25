fn main() {
    if let Err(e) = its_memory::cli_memory::run_cli("its-memory") {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
