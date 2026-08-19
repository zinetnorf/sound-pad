use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::Emitter;

use crate::models::AppConfig;

/// Conexión MIDI activa + flag de "aprender siguiente nota" (MIDI learn,
/// PLAN.md §7.6). Vive en `tauri::State`, gestionada una vez al arrancar la app.
pub struct MidiState {
    pub connection: Mutex<Option<midir::MidiInputConnection<()>>>,
    pub learning: Arc<AtomicBool>,
}

impl Default for MidiState {
    fn default() -> Self {
        Self { connection: Mutex::new(None), learning: Arc::new(AtomicBool::new(false)) }
    }
}

/// Enumera los puertos MIDI de entrada disponibles. Impura — no testeable
/// sin hardware, igual que `audio::devices::enumerate`.
pub fn list_devices() -> Vec<String> {
    let Ok(midi_in) = midir::MidiInput::new("soundpad-list") else { return Vec::new() };
    midi_in.ports().iter().filter_map(|p| midi_in.port_name(p).ok()).collect()
}

/// Abre una conexión al puerto indicado. Cada nota que llega: si hay un
/// MIDI learn pendiente, se emite `midi-learn-captured` y se apaga el flag;
/// si no, se busca el pad de esa nota en el banco activo y se dispara.
pub fn connect(
    port_name: &str,
    app: tauri::AppHandle,
    learning: Arc<AtomicBool>,
) -> Result<midir::MidiInputConnection<()>, String> {
    let midi_in = midir::MidiInput::new("soundpad-listen").map_err(|e| e.to_string())?;
    let ports = midi_in.ports();
    let port = ports
        .iter()
        .find(|p| midi_in.port_name(p).map(|n| n == port_name).unwrap_or(false))
        .ok_or_else(|| format!("puerto MIDI no encontrado: {port_name}"))?
        .clone();

    midi_in
        .connect(
            &port,
            "soundpad-input",
            move |_timestamp_us, message, _| {
                let Some(note) = parse_note_on(message) else { return };

                if learning.swap(false, Ordering::SeqCst) {
                    let _ = app.emit("midi-learn-captured", serde_json::json!({ "note": note }));
                    return;
                }

                let Ok(cfg_path) = crate::commands::config_path(&app) else { return };
                let Ok(config) = crate::storage::load_or_default(&cfg_path) else { return };
                if let Some((bank_id, pad_id)) = find_pad_by_note(&config, note) {
                    if let Err(e) = crate::commands::trigger(&app, &bank_id, &pad_id) {
                        eprintln!("disparo MIDI falló: {e}");
                    }
                }
            },
            (),
        )
        .map_err(|e| e.to_string())
}

/// Interpreta un mensaje MIDI crudo. Devuelve la nota solo si es un note-on
/// real con velocity > 0 — modo omni, el canal (nibble bajo del status) no
/// importa. note-off y note-on con velocity 0 (disfrazado) se descartan
/// (PLAN.md §9) — los pads son one-shot, igual que las teclas.
pub fn parse_note_on(message: &[u8]) -> Option<u8> {
    let &[status, note, velocity] = message else { return None };
    if status & 0xF0 == 0x90 && velocity > 0 {
        Some(note)
    } else {
        None
    }
}

/// Busca, dentro del banco activo, el pad con esta nota asignada (PLAN.md §9
/// — igual que los hotkeys, el alcance es por banco). Pura.
pub fn find_pad_by_note(config: &AppConfig, note: u8) -> Option<(String, String)> {
    let bank = config.banks.iter().find(|b| b.id == config.active_bank_id)?;
    let pad = bank.pads.iter().find(|p| p.midi_note == Some(note))?;
    Some((bank.id.clone(), pad.id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppConfig, Bank, Pad};

    #[test]
    fn parse_note_on_accepts_a_real_note_on_with_velocity() {
        assert_eq!(parse_note_on(&[0x90, 60, 100]), Some(60));
    }

    #[test]
    fn parse_note_on_is_omni_any_channel_counts() {
        // 0x9F = note-on, canal 15 (el nibble bajo del status es el canal).
        assert_eq!(parse_note_on(&[0x9F, 42, 1]), Some(42));
    }

    #[test]
    fn parse_note_on_rejects_velocity_zero_disguised_note_off() {
        assert_eq!(parse_note_on(&[0x90, 60, 0]), None);
    }

    #[test]
    fn parse_note_on_rejects_real_note_off() {
        assert_eq!(parse_note_on(&[0x80, 60, 100]), None);
    }

    #[test]
    fn parse_note_on_rejects_malformed_messages() {
        assert_eq!(parse_note_on(&[0x90, 60]), None);
        assert_eq!(parse_note_on(&[]), None);
    }

    fn pad_with_note(id: &str, note: u8) -> Pad {
        Pad { id: id.to_string(), name: id.to_string(), midi_note: Some(note), ..Pad::default() }
    }

    fn config_with_bank(pads: Vec<Pad>) -> AppConfig {
        AppConfig {
            active_bank_id: "b1".to_string(),
            banks: vec![Bank { id: "b1".to_string(), name: "Banco 1".to_string(), order: 0, pads }],
            ..AppConfig::default()
        }
    }

    #[test]
    fn find_pad_by_note_matches_within_the_active_bank() {
        let config = config_with_bank(vec![pad_with_note("a", 60)]);

        assert_eq!(find_pad_by_note(&config, 60), Some(("b1".to_string(), "a".to_string())));
    }

    #[test]
    fn find_pad_by_note_returns_none_when_no_pad_matches() {
        let config = config_with_bank(vec![pad_with_note("a", 60)]);

        assert_eq!(find_pad_by_note(&config, 61), None);
    }

    #[test]
    fn find_pad_by_note_ignores_pads_outside_the_active_bank() {
        let mut config = config_with_bank(vec![pad_with_note("a", 60)]);
        config.banks.push(Bank {
            id: "b2".to_string(),
            name: "Banco 2".to_string(),
            order: 1,
            pads: vec![pad_with_note("b", 61)],
        });

        assert_eq!(find_pad_by_note(&config, 61), None);
    }
}
