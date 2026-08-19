use serde::{Deserialize, Serialize};

/// Versión actual del esquema de `AppConfig`. Ver `storage::load_from_str`
/// para la política de migración (PLAN.md §5).
pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub banks: Vec<Bank>,
    pub active_bank_id: String,
    pub output_device: OutputDeviceRef,
    pub output_buffer_frames: u32,
    pub midi_device_name: Option<String>,
    pub target_lufs: f32,
    pub global_hotkeys_enabled: bool,
    pub global_modifier: String,
    /// `#[serde(default)]`: campo agregado después de la v1 — una config vieja
    /// en disco no lo tiene, y debe seguir cargando en vez de fallar.
    #[serde(default)]
    pub panic_hotkey: Option<String>,
    /// Idioma de la interfaz. Mismo criterio que `panic_hotkey`: config vieja
    /// en disco no trae el campo, cae al default en vez de fallar.
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "en".to_string()
}

impl AppConfig {
    /// Reordena los bancos tras un drag & drop de tabs (PLAN.md §10), igual
    /// que `Bank::reorder_pads` pero un nivel arriba.
    pub fn reorder_banks(&mut self, ordered_ids: &[String]) {
        let mut remaining = std::mem::take(&mut self.banks);
        let mut new_banks = Vec::with_capacity(remaining.len());

        for id in ordered_ids {
            if let Some(pos) = remaining.iter().position(|b| &b.id == id) {
                new_banks.push(remaining.remove(pos));
            }
        }
        new_banks.extend(remaining);

        for (i, bank) in new_banks.iter_mut().enumerate() {
            bank.order = i as u32;
        }
        self.banks = new_banks;
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            banks: Vec::new(),
            active_bank_id: String::new(),
            output_device: OutputDeviceRef::default(),
            output_buffer_frames: 256,
            midi_device_name: None,
            target_lufs: -16.0,
            global_hotkeys_enabled: true,
            global_modifier: "Ctrl+Alt".to_string(),
            panic_hotkey: None,
            language: default_language(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDeviceRef {
    pub device_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    pub id: String,
    pub name: String,
    pub order: u32,
    pub pads: Vec<Pad>,
}

impl Bank {
    /// Reemplaza el pad cuyo id coincide con `updated.id`. Usado por el modal
    /// de editar y por los controles inline (trim, loop) del modo Config.
    pub fn replace_pad(&mut self, updated: Pad) -> bool {
        match self.pads.iter_mut().find(|p| p.id == updated.id) {
            Some(slot) => {
                *slot = updated;
                true
            }
            None => false,
        }
    }

    /// Quita el pad y lo devuelve, para que el caller borre también su
    /// archivo de audio (PLAN.md §7.6: eliminar borra el WAV del disco).
    pub fn remove_pad(&mut self, pad_id: &str) -> Option<Pad> {
        let index = self.pads.iter().position(|p| p.id == pad_id)?;
        Some(self.pads.remove(index))
    }

    /// Reordena según `ordered_ids` (drag & drop, PLAN.md §7.2) y reasigna
    /// `order` secuencialmente. Ids que falten en la lista se agregan al final,
    /// preservando su orden relativo, para no perder pads por una lista incompleta.
    pub fn reorder_pads(&mut self, ordered_ids: &[String]) {
        let mut remaining = std::mem::take(&mut self.pads);
        let mut new_pads = Vec::with_capacity(remaining.len());

        for id in ordered_ids {
            if let Some(pos) = remaining.iter().position(|p| &p.id == id) {
                new_pads.push(remaining.remove(pos));
            }
        }
        new_pads.extend(remaining);

        for (i, pad) in new_pads.iter_mut().enumerate() {
            pad.order = i as u32;
        }
        self.pads = new_pads;
    }

    /// Rechaza la tecla si otro pad del banco ya la tiene asignada
    /// (PLAN.md §8 — los atajos solo tienen sentido dentro de un banco).
    pub fn validate_hotkey(&self, candidate: &Pad) -> Result<(), String> {
        let Some(hotkey) = &candidate.hotkey else { return Ok(()) };
        match self.pads.iter().find(|p| p.id != candidate.id && p.hotkey.as_deref() == Some(hotkey.as_str())) {
            Some(conflict) => Err(format!("La tecla \"{hotkey}\" ya está asignada a \"{}\"", conflict.name)),
            None => Ok(()),
        }
    }

    /// Mismo criterio que `validate_hotkey`, para notas MIDI (PLAN.md §9:
    /// "las asignaciones MIDI, como los hotkeys, solo responden al banco activo").
    pub fn validate_midi_note(&self, candidate: &Pad) -> Result<(), String> {
        let Some(note) = candidate.midi_note else { return Ok(()) };
        match self.pads.iter().find(|p| p.id != candidate.id && p.midi_note == Some(note)) {
            Some(conflict) => Err(format!("La nota MIDI {note} ya está asignada a \"{}\"", conflict.name)),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pad {
    pub id: String,
    pub name: String,
    pub order: u32,
    pub color: String,
    pub trim_db: f32,
    pub loop_enabled: bool,
    pub retrigger_mode: RetriggerMode,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub hotkey: Option<String>,
    pub midi_note: Option<u8>,
    pub duration_ms: u32,

    pub original_filename: String,
    pub source_sample_rate: u32,
    pub source_channels: u8,
    pub measured_lufs: f32,
    pub applied_gain_db: f32,
    pub gain_capped: bool,
}

impl Default for Pad {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            order: 0,
            color: "#E8734A".to_string(),
            trim_db: 0.0,
            loop_enabled: false,
            retrigger_mode: RetriggerMode::Stop,
            fade_in_ms: 20,
            fade_out_ms: 20,
            hotkey: None,
            midi_note: None,
            duration_ms: 0,
            original_filename: String::new(),
            source_sample_rate: 0,
            source_channels: 0,
            measured_lufs: 0.0,
            applied_gain_db: 0.0,
            gain_capped: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RetriggerMode {
    #[default]
    Stop,
    Restart,
    Ignore,
    Overlap,
}

#[cfg(test)]
mod bank_ops_tests {
    use super::*;

    fn pad_with(id: &str, order: u32) -> Pad {
        Pad { id: id.to_string(), name: id.to_string(), order, ..Pad::default() }
    }

    fn bank_with(pads: Vec<Pad>) -> Bank {
        Bank { id: "b1".to_string(), name: "Banco 1".to_string(), order: 0, pads }
    }

    #[test]
    fn replace_pad_updates_matching_pad_and_returns_true() {
        let mut bank = bank_with(vec![pad_with("a", 0), pad_with("b", 1)]);
        let mut updated = pad_with("a", 0);
        updated.name = "Nuevo nombre".to_string();
        updated.trim_db = 3.0;

        let found = bank.replace_pad(updated.clone());

        assert!(found);
        assert_eq!(bank.pads[0].name, "Nuevo nombre");
        assert_eq!(bank.pads[0].trim_db, 3.0);
        assert_eq!(bank.pads[1].name, "b");
    }

    #[test]
    fn replace_pad_returns_false_when_id_not_found() {
        let mut bank = bank_with(vec![pad_with("a", 0)]);
        let unknown = pad_with("z", 0);

        let found = bank.replace_pad(unknown);

        assert!(!found);
        assert_eq!(bank.pads.len(), 1);
    }

    #[test]
    fn remove_pad_removes_matching_pad_and_returns_it() {
        let mut bank = bank_with(vec![pad_with("a", 0), pad_with("b", 1)]);

        let removed = bank.remove_pad("a");

        assert_eq!(removed.map(|p| p.id), Some("a".to_string()));
        assert_eq!(bank.pads.len(), 1);
        assert_eq!(bank.pads[0].id, "b");
    }

    #[test]
    fn remove_pad_returns_none_when_id_not_found() {
        let mut bank = bank_with(vec![pad_with("a", 0)]);

        let removed = bank.remove_pad("z");

        assert_eq!(removed, None);
        assert_eq!(bank.pads.len(), 1);
    }

    #[test]
    fn reorder_pads_applies_new_order_and_reindexes() {
        let mut bank = bank_with(vec![pad_with("a", 0), pad_with("b", 1), pad_with("c", 2)]);

        bank.reorder_pads(&["c".to_string(), "a".to_string(), "b".to_string()]);

        let ids: Vec<&str> = bank.pads.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["c", "a", "b"]);
        assert_eq!(bank.pads[0].order, 0);
        assert_eq!(bank.pads[1].order, 1);
        assert_eq!(bank.pads[2].order, 2);
    }

    #[test]
    fn reorder_pads_appends_ids_missing_from_the_new_order() {
        // Defensivo: si el frontend manda una lista incompleta, no perder pads.
        let mut bank = bank_with(vec![pad_with("a", 0), pad_with("b", 1), pad_with("c", 2)]);

        bank.reorder_pads(&["c".to_string()]);

        let ids: Vec<&str> = bank.pads.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["c", "a", "b"]);
    }

    fn pad_with_hotkey(id: &str, hotkey: &str) -> Pad {
        Pad { id: id.to_string(), name: id.to_string(), hotkey: Some(hotkey.to_string()), ..Pad::default() }
    }

    #[test]
    fn validate_hotkey_rejects_a_key_already_used_by_another_pad() {
        let bank = bank_with(vec![pad_with_hotkey("a", "K")]);
        let candidate = pad_with_hotkey("b", "K");

        let result = bank.validate_hotkey(&candidate);

        assert!(result.is_err());
    }

    #[test]
    fn validate_hotkey_allows_a_pad_to_keep_its_own_key() {
        let bank = bank_with(vec![pad_with_hotkey("a", "K")]);
        let same_pad_edited = pad_with_hotkey("a", "K");

        assert!(bank.validate_hotkey(&same_pad_edited).is_ok());
    }

    #[test]
    fn validate_hotkey_allows_an_unused_key() {
        let bank = bank_with(vec![pad_with_hotkey("a", "K")]);
        let candidate = pad_with_hotkey("b", "Z");

        assert!(bank.validate_hotkey(&candidate).is_ok());
    }

    #[test]
    fn validate_hotkey_allows_no_hotkey_at_all() {
        let bank = bank_with(vec![pad_with_hotkey("a", "K")]);
        let candidate = pad_with("b", 1);

        assert!(bank.validate_hotkey(&candidate).is_ok());
    }

    fn pad_with_note(id: &str, note: u8) -> Pad {
        Pad { id: id.to_string(), name: id.to_string(), midi_note: Some(note), ..Pad::default() }
    }

    #[test]
    fn validate_midi_note_rejects_a_note_already_used_by_another_pad() {
        let bank = bank_with(vec![pad_with_note("a", 60)]);
        let candidate = pad_with_note("b", 60);

        assert!(bank.validate_midi_note(&candidate).is_err());
    }

    #[test]
    fn validate_midi_note_allows_a_pad_to_keep_its_own_note() {
        let bank = bank_with(vec![pad_with_note("a", 60)]);
        let same_pad_edited = pad_with_note("a", 60);

        assert!(bank.validate_midi_note(&same_pad_edited).is_ok());
    }

    #[test]
    fn validate_midi_note_allows_no_note_at_all() {
        let bank = bank_with(vec![pad_with_note("a", 60)]);
        let candidate = pad_with("b", 1);

        assert!(bank.validate_midi_note(&candidate).is_ok());
    }

    fn named_bank(id: &str, order: u32) -> Bank {
        Bank { id: id.to_string(), name: id.to_string(), order, pads: Vec::new() }
    }

    #[test]
    fn reorder_banks_applies_new_order_and_reindexes() {
        let mut config = AppConfig { banks: vec![named_bank("a", 0), named_bank("b", 1), named_bank("c", 2)], ..AppConfig::default() };

        config.reorder_banks(&["c".to_string(), "a".to_string(), "b".to_string()]);

        let ids: Vec<&str> = config.banks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["c", "a", "b"]);
        assert_eq!(config.banks[0].order, 0);
        assert_eq!(config.banks[2].order, 2);
    }

    #[test]
    fn reorder_banks_appends_ids_missing_from_the_new_order() {
        let mut config = AppConfig { banks: vec![named_bank("a", 0), named_bank("b", 1)], ..AppConfig::default() };

        config.reorder_banks(&["b".to_string()]);

        let ids: Vec<&str> = config.banks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_default_matches_documented_values() {
        let pad = Pad::default();

        assert_eq!(pad.trim_db, 0.0);
        assert!(!pad.loop_enabled);
        assert_eq!(pad.retrigger_mode, RetriggerMode::Stop);
        assert_eq!(pad.fade_in_ms, 20);
        assert_eq!(pad.fade_out_ms, 20);
        assert_eq!(pad.hotkey, None);
        assert_eq!(pad.midi_note, None);
        assert!(!pad.gain_capped);
    }

    #[test]
    fn app_config_default_matches_documented_values() {
        let config = AppConfig::default();

        assert_eq!(config.version, CURRENT_VERSION);
        assert_eq!(config.output_buffer_frames, 256);
        assert_eq!(config.target_lufs, -16.0);
        assert!(config.global_hotkeys_enabled);
        assert_eq!(config.global_modifier, "Ctrl+Alt");
        assert!(config.banks.is_empty());
        assert_eq!(config.language, "en");
    }
}
