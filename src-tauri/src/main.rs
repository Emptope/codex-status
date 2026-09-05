#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--collector") {
        let path = args
            .get(2)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("codex-status-preview.json"));
        tokio::runtime::Runtime::new()
            .expect("Runtime unavailable")
            .block_on(collect(path));
        return;
    }
    #[cfg(feature = "desktop")]
    codex_status::platform::run();
    #[cfg(not(feature = "desktop"))]
    eprintln!("Build with the desktop feature to open the window.");
}

async fn collect(path: std::path::PathBuf) {
    use codex_status::{settings::Settings, sources::Runtime};
    use std::io::Write;
    use tokio::io::AsyncBufReadExt;
    let runtime = Runtime::new(path, |snapshot| {
        if let Ok(line) = serde_json::to_string(&snapshot) {
            let mut output = std::io::stdout().lock();
            let _ = writeln!(output, "{line}");
            let _ = output.flush();
        }
    });
    runtime.start();
    let mut input = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = input.next_line().await {
        if line.len() > 65536 {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        match value["command"].as_str() {
            Some("refresh") => runtime.request_refresh(),
            Some("save_preferences") => {
                if let Ok(settings) = serde_json::from_value::<Settings>(value["settings"].clone())
                {
                    let _ = runtime.update_settings(settings);
                }
            }
            _ => {}
        }
    }
    if !runtime.close().await {
        std::process::exit(0);
    }
}
