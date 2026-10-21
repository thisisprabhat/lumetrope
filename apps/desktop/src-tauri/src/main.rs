// Lumetrope desktop: hosts the web editor in a native window with native
// file dialogs and filesystem access. All editing runs in the WASM engine.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .run(tauri::generate_context!())
        .expect("error while running Lumetrope");
}
