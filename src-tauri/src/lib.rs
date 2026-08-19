pub mod audio;
pub mod commands;
pub mod export;
pub mod hotkeys;
pub mod midi;
pub mod models;
pub mod seed;
pub mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Err(e) = commands::init_engine(&app.handle().clone()) {
                // No tirar la app abajo si no hay dispositivo de salida al arrancar;
                // el usuario puede elegir uno en Configuración más adelante.
                eprintln!("init_engine falló: {e}");
            }
            if let Err(e) = commands::init_midi(&app.handle().clone()) {
                eprintln!("init_midi falló: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_output_devices,
            commands::play_test_tone,
            commands::get_config,
            commands::save_config,
            commands::import_pad,
            commands::update_pad,
            commands::delete_pad,
            commands::reorder_pads,
            commands::trigger_pad,
            commands::stop_pad,
            commands::panic,
            commands::create_bank,
            commands::rename_bank,
            commands::delete_bank,
            commands::reorder_banks,
            commands::set_active_bank,
            commands::list_midi_devices,
            commands::set_midi_device,
            commands::start_midi_learn,
            commands::cancel_midi_learn,
            commands::set_output_device,
            commands::export_config,
            commands::import_config,
            commands::renormalize_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
