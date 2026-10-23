fn main() {
    let cli = ia_uploader::Cli::parse_localized();
    let lang = cli.language();
    if let Err(e) = ia_uploader::run(cli) {
        eprintln!("{}: {e:#}", lang.text("错误", "Error"));
        std::process::exit(1);
    }
}
