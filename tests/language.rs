use std::process::Command;
fn cli(language: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ia-upload-cli"))
        .env("IA_UPLOADER_LANG", language)
        .env_remove("IA_ACCESS_KEY")
        .env_remove("IA_SECRET_KEY")
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn environment_controls_help_and_runtime_errors() {
    for (language, help, error) in [
        ("en", "Proxy URL", "Error: Set IA_ACCESS_KEY"),
        ("zh", "手动代理地址", "错误: 请设置 IA_ACCESS_KEY"),
    ] {
        let output = cli(language, &["--help"]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains(help));
        let output = cli(language, &["auth"]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(error));
    }
}
#[test]
fn explicit_flag_overrides_environment_for_subcommand_help() {
    let output = cli("zh", &["upload", "--lang=en", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("Additional retry attempts per file"));
    assert!(!help.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)));
    let output = cli("en", &["--lang", "zh", "auth", "--help"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("验证 IA_ACCESS_KEY"));
}
