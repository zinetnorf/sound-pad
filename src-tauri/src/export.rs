use std::io::{Read, Write};
use std::path::Path;

use thiserror::Error;

use crate::storage;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("error de zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("el zip no contiene config.json")]
    MissingConfig,
    #[error("config.json no es UTF-8 válido: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("config inválida: {0}")]
    InvalidConfig(#[from] storage::StorageError),
}

/// Empaqueta `config.json` + `audio/` en un `.zip` (PLAN.md §11).
pub fn export_zip(config_path: &Path, audio_dir: &Path, dest_zip: &Path) -> Result<(), ExportError> {
    let file = std::fs::File::create(dest_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("config.json", options)?;
    zip.write_all(&std::fs::read(config_path)?)?;

    if audio_dir.is_dir() {
        for entry in std::fs::read_dir(audio_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = format!("audio/{}", entry.file_name().to_string_lossy());
            zip.start_file(name, options)?;
            zip.write_all(&std::fs::read(entry.path())?)?;
        }
    }
    zip.finish()?;
    Ok(())
}

/// Restaura `config.json` + `audio/` desde un `.zip`. Valida la versión del
/// esquema ANTES de escribir nada en disco (PLAN.md §11) — un zip corrupto o
/// de una versión futura no debe dejar la instalación a medio migrar.
pub fn import_zip(source_zip: &Path, config_path: &Path, audio_dir: &Path) -> Result<(), ExportError> {
    let file = std::fs::File::open(source_zip)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let config_bytes = {
        let mut entry = archive.by_name("config.json").map_err(|_| ExportError::MissingConfig)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        buf
    };
    let json = String::from_utf8(config_bytes.clone())?;
    storage::load_from_str(&json)?;

    std::fs::create_dir_all(audio_dir)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(fname) = entry.name().strip_prefix("audio/").map(str::to_string) else { continue };
        if fname.is_empty() || entry.is_dir() {
            continue;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        std::fs::write(audio_dir.join(&fname), buf)?;
    }

    std::fs::write(config_path, config_bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, r#"{"version":1,"banks":[],"activeBankId":"","outputDevice":{"deviceId":"","label":""},"outputBufferFrames":256,"midiDeviceName":null,"targetLufs":-16.0,"globalHotkeysEnabled":true,"globalModifier":"Ctrl+Alt"}"#).unwrap();
        let audio_dir = dir.join("audio");
        std::fs::create_dir_all(&audio_dir).unwrap();
        std::fs::write(audio_dir.join("pad-1.wav"), b"fake wav bytes").unwrap();
        (config_path, audio_dir)
    }

    #[test]
    fn export_then_import_round_trips_config_and_audio_files() {
        let src_dir = tempfile::tempdir().unwrap();
        let (config_path, audio_dir) = write_fixture(src_dir.path());
        let zip_path = src_dir.path().join("export.zip");

        export_zip(&config_path, &audio_dir, &zip_path).unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let dest_config = dest_dir.path().join("config.json");
        let dest_audio = dest_dir.path().join("audio");
        import_zip(&zip_path, &dest_config, &dest_audio).unwrap();

        assert_eq!(std::fs::read_to_string(&dest_config).unwrap(), std::fs::read_to_string(&config_path).unwrap());
        assert_eq!(std::fs::read(dest_audio.join("pad-1.wav")).unwrap(), b"fake wav bytes");
    }

    #[test]
    fn import_rejects_a_future_schema_version_without_touching_the_destination() {
        let src_dir = tempfile::tempdir().unwrap();
        let audio_dir = src_dir.path().join("audio");
        std::fs::create_dir_all(&audio_dir).unwrap();
        std::fs::write(audio_dir.join("pad-1.wav"), b"fake").unwrap();
        let config_path = src_dir.path().join("config.json");
        std::fs::write(&config_path, r#"{"version":999}"#).unwrap();
        let zip_path = src_dir.path().join("export.zip");
        export_zip(&config_path, &audio_dir, &zip_path).unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let dest_config = dest_dir.path().join("config.json");
        let dest_audio = dest_dir.path().join("audio");

        let result = import_zip(&zip_path, &dest_config, &dest_audio);

        assert!(result.is_err());
        assert!(!dest_config.exists());
        assert!(!dest_audio.join("pad-1.wav").exists());
    }

    #[test]
    fn import_fails_cleanly_when_the_zip_has_no_config_json() {
        let src_dir = tempfile::tempdir().unwrap();
        let zip_path = src_dir.path().join("empty.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"no config here").unwrap();
        zip.finish().unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let result = import_zip(&zip_path, &dest_dir.path().join("config.json"), &dest_dir.path().join("audio"));

        assert!(matches!(result, Err(ExportError::MissingConfig)));
    }
}
