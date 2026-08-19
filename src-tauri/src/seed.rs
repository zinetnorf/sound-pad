use std::path::Path;

use tauri::Manager;

use crate::audio::normalize;
use crate::models::{AppConfig, Bank, Pad};

/// Color azul por defecto para los pads del banco semilla (segundo swatch del
/// picker del frontend, ver `DEFAULT_COLORS` en AddPadModal/PadEditModal).
const SEED_PAD_COLOR: &str = "#4AA3E8";

/// Pads de fábrica del banco "Default", en el orden en que deben quedar en
/// la grilla. El segundo elemento es el archivo empaquetado bajo
/// `resources/default-pads/` (ver `tauri.conf.json` → `bundle.resources`).
const DEFAULT_PADS: &[(&str, &str)] = &[
    ("Risas 1", "risas1.mp3"),
    ("Risas 2", "risas2.mp3"),
    ("Aplausos 1", "aplausos1.mp3"),
    ("Aplausos 2", "aplausos2.mp3"),
    ("Abucheo (buu)", "abucheo.mp3"),
    ("Drum Roll", "drum-roll.mp3"),
    ("Ta-Da", "tada.mp3"),
    ("Wa-wa-wa", "wa-wa-wa.mp3"),
    ("Ba-dum-tss", "ba-dum-tss.mp3"),
];

/// Genera el `AppConfig` de primer arranque: un único banco "Default" con los
/// 9 pads de fábrica, corriendo cada mp3 empaquetado por el mismo pipeline de
/// normalización que usa `import_pad` (PLAN.md §6.3). Solo debe llamarse
/// cuando `config.json` todavía no existe (primera instalación).
pub fn seed_default_config(app: &tauri::AppHandle, audio_dir: &Path) -> Result<AppConfig, String> {
    let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    let pads_dir = resource_dir.join("default-pads");

    let target_lufs = AppConfig::default().target_lufs;
    let mut pads = Vec::with_capacity(DEFAULT_PADS.len());

    for (order, (name, filename)) in DEFAULT_PADS.iter().enumerate() {
        let source_path = pads_dir.join(filename);
        let result =
            normalize::import_pipeline(&source_path, target_lufs).map_err(|e| format!("{filename}: {e}"))?;
        let duration_ms = (result.stereo_48k.len() / 2) as f64 / normalize::TARGET_SAMPLE_RATE as f64 * 1000.0;

        let pad = Pad {
            name: name.to_string(),
            order: order as u32,
            color: SEED_PAD_COLOR.to_string(),
            duration_ms: duration_ms.round() as u32,
            original_filename: filename.to_string(),
            source_sample_rate: result.source_sample_rate,
            source_channels: result.source_channels,
            measured_lufs: result.measured_lufs,
            applied_gain_db: result.applied_gain_db,
            gain_capped: result.gain_capped,
            ..Pad::default()
        };

        let wav_path = audio_dir.join(format!("{}.wav", pad.id));
        normalize::write_wav(&wav_path, &result.stereo_48k).map_err(|e| e.to_string())?;

        pads.push(pad);
    }

    let bank = Bank { id: uuid::Uuid::new_v4().to_string(), name: "Default".to_string(), order: 0, pads };
    Ok(AppConfig { active_bank_id: bank.id.clone(), banks: vec![bank], ..AppConfig::default() })
}
