fn main() {
    if let Err(e) = its_memory::cli_coin::run_cli("its-coin") {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
