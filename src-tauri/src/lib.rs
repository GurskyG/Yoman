#[cfg(test)]
mod access_policy;
mod commands;
mod entry;
mod journal;
mod settings;
mod write;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::journal_status,
            commands::choose_journal_folder,
            commands::read_entry,
            commands::create_entry,
            commands::save_entry,
            commands::delete_entry
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
