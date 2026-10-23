use clap::Parser;
fn main() {
    if let Err(e) = ia_uploader::run(ia_uploader::Cli::parse()) {
        eprintln!("错误：{e:#}");
        std::process::exit(1);
    }
}
