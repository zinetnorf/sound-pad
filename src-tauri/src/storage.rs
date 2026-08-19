use std::path::Path;

use thiserror::Error;

use crate::models::{AppConfig, CURRENT_VERSION};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("no se pudo acceder al archivo de configuración: {0}")]
    Io(#[from] std::io::Error),
    #[error("config corrupta: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("la config es de la versión {found}; esta app soporta hasta la versión {current}")]
    FutureVersion { found: u32, current: u32 },
}

/// Parsea y valida la versión de un `AppConfig` desde JSON crudo.
/// Rechaza versiones futuras en vez de intentar adivinar el esquema (PLAN.md §5).
pub fn load_from_str(json: &str) -> Result<AppConfig, StorageError> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let found_version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    if found_version > CURRENT_VERSION {
        return Err(StorageError::FutureVersion { found: found_version, current: CURRENT_VERSION });
    }

    let migrated = migrate(value, found_version);
    Ok(serde_json::from_value(migrated)?)
}

/// Cadena de migraciones. Hoy no hay ninguna (CURRENT_VERSION = 1);
/// cuando el esquema cambie, agregar un brazo por versión de origen.
fn migrate(value: serde_json::Value, _from_version: u32) -> serde_json::Value {
    value
}

/// Carga la config desde disco. Si el archivo no existe todavía
/// (primer arranque), devuelve una config nueva válida en vez de fallar.
pub fn load_or_default(path: &Path) -> Result<AppConfig, StorageError> {
    match std::fs::read_to_string(path) {
        Ok(json) => load_from_str(&json),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(e) => Err(e.into()),
    }
}

/// Escritura atómica: escribe a un archivo temporal y lo renombra.
/// Evita dejar `config.json` corrupto si el proceso se corta a mitad de la escritura.
pub fn save(path: &Path, config: &AppConfig) -> Result<(), StorageError> {
    let json = serde_json::to_string_pretty(config)?;
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp_path = std::path::PathBuf::from(tmp_name);

    std::fs::write(&tmp_path, json)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Bank, OutputDeviceRef, Pad, RetriggerMode};

    fn sample_config() -> AppConfig {
        AppConfig {
            active_bank_id: "bank-1".to_string(),
            output_device: OutputDeviceRef { device_id: "usb-mixer".to_string(), label: "USB Mixer".to_string() },
            banks: vec![Bank {
                id: "bank-1".to_string(),
                name: "Podcast 1".to_string(),
                order: 0,
                pads: vec![Pad {
                    name: "Aplausos".to_string(),
                    retrigger_mode: RetriggerMode::Overlap,
                    ..Pad::default()
                }],
            }],
            ..AppConfig::default()
        }
    }

    #[test]
    fn rejects_config_from_a_future_version() {
        let json = r#"{"version": 999}"#;

        let result = load_from_str(json);

        assert!(matches!(result, Err(StorageError::FutureVersion { found: 999, current: CURRENT_VERSION })));
    }

    #[test]
    fn round_trips_a_full_config_through_json() {
        let original = sample_config();
        let json = serde_json::to_string(&original).unwrap();

        let loaded = load_from_str(&json).unwrap();

        assert_eq!(loaded, original);
    }

    #[test]
    fn rejects_corrupt_json_without_panicking() {
        let result = load_from_str("{ esto no es json valido");

        assert!(matches!(result, Err(StorageError::Parse(_))));
    }

    #[test]
    fn loads_a_v1_config_written_before_panic_hotkey_existed() {
        // panicHotkey se agregó después de la v1 sin subir CURRENT_VERSION;
        // una config vieja en disco no lo tiene y debe seguir cargando.
        let json = r#"{
            "version": 1, "banks": [], "activeBankId": "", "outputBufferFrames": 256,
            "outputDevice": {"deviceId": "", "label": ""}, "midiDeviceName": null,
            "targetLufs": -16.0, "globalHotkeysEnabled": true, "globalModifier": "Ctrl+Alt"
        }"#;

        let config = load_from_str(json).unwrap();

        assert_eq!(config.panic_hotkey, None);
    }

    #[test]
    fn load_or_default_returns_fresh_config_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = load_or_default(&path).unwrap();

        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn save_then_load_round_trips_and_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let original = sample_config();

        save(&path, &original).unwrap();
        let loaded = load_or_default(&path).unwrap();

        assert_eq!(loaded, original);
        assert!(!path.with_extension("json.tmp").exists());
        let mut tmp_name = path.as_os_str().to_owned();
        tmp_name.push(".tmp");
        assert!(!std::path::Path::new(&tmp_name).exists());
    }
}
