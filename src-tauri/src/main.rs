#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(profile_id) = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|args| args[0] == "--codex-provider-token")
        .map(|args| args[1].clone())
    {
        match print_provider_token(&profile_id) {
            Ok(()) => std::process::exit(0),
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(1);
            }
        }
    }

    codex_api_switcher_core::run();
}

fn print_provider_token(profile_id: &str) -> Result<(), String> {
    use std::io::Write;

    use codex_api_switcher_core::switcher::state::SwitcherPaths;
    use codex_api_switcher_core::switcher::store::SwitcherStore;

    let paths = SwitcherPaths::from_system().map_err(|error| error.message)?;
    let store = SwitcherStore::new(&paths);
    let database = store.load().map_err(|error| error.message)?;
    let profile = database
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "没有找到 Provider 凭据。".to_string())?;
    let token = store
        .provider_secret(profile)
        .map_err(|error| error.message)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(token.as_bytes())
        .and_then(|_| stdout.flush())
        .map_err(|error| format!("输出 Provider 凭据失败：{error}"))
}
