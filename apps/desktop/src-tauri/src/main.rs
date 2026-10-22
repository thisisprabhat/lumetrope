// Lumetrope desktop: hosts the web editor in a native window with native
// file dialogs and filesystem access. All editing runs in the WASM engine.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use tauri::path::BaseDirectory;
use tauri::Manager;

/// Absolute path of the bundled MCP server, copied out of the app bundle.
struct McpServer(Result<PathBuf, String>);

/// Copy the bundled MCP server to `<app data>/mcp/` so agents get a path that
/// survives app updates, moving the app and AppImage remounts.
fn install_mcp_server(app: &tauri::App) -> Result<PathBuf, String> {
    let src = app.path().resolve("mcp/lumetrope-mcp.mjs", BaseDirectory::Resource).map_err(|e| e.to_string())?;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("mcp");
    let dest = dir.join("lumetrope-mcp.mjs");
    let bytes = std::fs::read(&src).map_err(|e| format!("bundled MCP server missing at {}: {e}", src.display()))?;
    if std::fs::read(&dest).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    }
    Ok(strip_verbatim(&dest))
}

/// `\\?\C:\…` → `C:\…` so the path pastes cleanly into shells and configs.
fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

#[tauri::command]
fn mcp_server_path(state: tauri::State<McpServer>) -> Result<String, String> {
    state.0.as_ref().map(|p| p.to_string_lossy().into_owned()).map_err(Clone::clone)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // Remembers files picked in dialogs so recent files reopen after a restart.
        .plugin(tauri_plugin_persisted_scope::init())
        .setup(|app| {
            let installed = install_mcp_server(app);
            app.manage(McpServer(installed));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![mcp_server_path])
        .run(tauri::generate_context!())
        .expect("error while running Lumetrope");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_windows_verbatim_prefix_only() {
        assert_eq!(strip_verbatim(Path::new(r"\\?\C:\Users\me\x.mjs")), PathBuf::from(r"C:\Users\me\x.mjs"));
        assert_eq!(strip_verbatim(Path::new(r"\\?\UNC\server\share\x.mjs")), PathBuf::from(r"\\?\UNC\server\share\x.mjs"));
        assert_eq!(strip_verbatim(Path::new("/Users/me/x.mjs")), PathBuf::from("/Users/me/x.mjs"));
    }
}
