use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::DeviceTrait;
use kira::{
    backend::cpal::{CpalBackend, CpalBackendSettings},
    sound::{
        static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
        PlaybackState,
    },
    AudioManager, AudioManagerSettings, Decibels, Frame, Tween,
};
use thiserror::Error;

use super::devices::{negotiate_config, ConfigChoice, SupportedRange};
use super::playback::{decide_trigger_action, TriggerAction};
use crate::models::Pad;

pub const TARGET_SAMPLE_RATE: u32 = 48_000;
pub const TARGET_BUFFER_FRAMES: u32 = 256;
/// Tope de voces simultáneas por pad en modo Overlap (PLAN.md §6.5).
const MAX_OVERLAP_VOICES: usize = 8;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("no se pudo abrir el dispositivo de audio: {0}")]
    Backend(String),
    #[error("pad no precargado: {0}")]
    NotPreloaded(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayOutcome {
    /// Arrancó una voz nueva (disparo normal, restart, u overlap).
    Started,
    /// Modo Stop mientras ya sonaba: se detuvo, no arrancó nada nuevo.
    StoppedOnly,
    /// Modo Ignore mientras ya sonaba: no pasó nada.
    Ignored,
}

pub struct PadTick {
    pub pad_id: String,
    pub remaining_ms: i64,
}

/// Motor de audio. Recibe el dispositivo por parámetro de construcción
/// (nunca asume el default) para permitir un segundo motor de Cue a futuro. Ver PLAN.md §6.2.
pub struct AudioEngine {
    manager: AudioManager<CpalBackend>,
    preloaded: HashMap<String, StaticSoundData>,
    /// Voces activas por pad, en orden de disparo (la más vieja primero) —
    /// así el modo Overlap sabe a cuál expulsar al llegar al tope.
    active: HashMap<String, Vec<(StaticSoundHandle, u32)>>,
    /// Voz del preview del editor de recorte. Separada de `active` a
    /// propósito: no es un pad, no debe aparecer en `poll()`/`pad-tick` ni
    /// contar para el tope de voces de ningún pad (SPEC-recorte-v2.md).
    preview: Option<StaticSoundHandle>,
}

impl AudioEngine {
    pub fn new(device: cpal::Device) -> Result<Self, EngineError> {
        let config = negotiated_stream_config(&device);
        let manager = AudioManager::<CpalBackend>::new(AudioManagerSettings {
            backend_settings: CpalBackendSettings {
                device: Some(device),
                config: Some(config),
            },
            ..Default::default()
        })
        .map_err(|e| EngineError::Backend(e.to_string()))?;
        Ok(Self { manager, preloaded: HashMap::new(), active: HashMap::new(), preview: None })
    }

    /// Reproduce un tono senoidal corto. Valida que el audio sale por el
    /// dispositivo elegido — es el hito de la Fase 1 (PLAN.md §13).
    pub fn play_test_tone(&mut self) -> Result<(), EngineError> {
        let data = test_tone(440.0, 0.5, TARGET_SAMPLE_RATE);
        self.manager
            .play(data)
            .map_err(|e| EngineError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Carga a memoria los WAV normalizados de estos pads. El primer disparo
    /// de cualquiera nunca debe decodificar ni leer disco (PLAN.md §6.5).
    pub fn preload(&mut self, pads: &[Pad], audio_dir: &std::path::Path) -> Result<(), EngineError> {
        self.preloaded.clear();
        for pad in pads {
            let path = audio_dir.join(format!("{}.wav", pad.id));
            let data = StaticSoundData::from_file(&path).map_err(|e| EngineError::Backend(e.to_string()))?;
            self.preloaded.insert(pad.id.clone(), data);
        }
        Ok(())
    }

    /// Dispara un pad aplicando su modo de re-disparo (PLAN.md §6.5, §7 tooltip).
    pub fn play(&mut self, pad: &Pad) -> Result<PlayOutcome, EngineError> {
        let active_count = self.active.get(&pad.id).map(|v| v.len()).unwrap_or(0);
        let action = decide_trigger_action(pad.retrigger_mode, active_count, MAX_OVERLAP_VOICES);
        let fade_out = tween_ms(pad.fade_out_ms);

        match action {
            TriggerAction::DoNothing => Ok(PlayOutcome::Ignored),
            TriggerAction::StopOnly => {
                self.stop_voices_for(&pad.id, fade_out);
                Ok(PlayOutcome::StoppedOnly)
            }
            TriggerAction::RestartFromBeginning => {
                self.stop_voices_for(&pad.id, fade_out);
                self.start_voice(pad)?;
                Ok(PlayOutcome::Started)
            }
            TriggerAction::AddOverlap { evict_oldest } => {
                if evict_oldest {
                    if let Some(voices) = self.active.get_mut(&pad.id) {
                        if !voices.is_empty() {
                            let (mut oldest, _) = voices.remove(0);
                            oldest.stop(fade_out);
                        }
                    }
                }
                self.start_voice(pad)?;
                Ok(PlayOutcome::Started)
            }
            TriggerAction::Start => {
                self.start_voice(pad)?;
                Ok(PlayOutcome::Started)
            }
        }
    }

    /// Detiene manualmente un pad con su fade-out configurado, para no chasquear.
    pub fn stop(&mut self, pad_id: &str, fade_out_ms: u32) {
        self.stop_voices_for(pad_id, tween_ms(fade_out_ms));
    }

    /// Botón de pánico: detiene todo, todos los bancos, con fade-out corto.
    /// Devuelve los pads que estaban sonando, para que el caller emita `pad-stopped`
    /// de cada uno — si no, la UI se queda con el contador congelado (nunca se entera).
    /// También corta el preview del editor de recorte si estaba sonando.
    pub fn stop_all(&mut self, fade_out_ms: u32) -> Vec<String> {
        let tween = tween_ms(fade_out_ms);
        let ids: Vec<String> = self.active.keys().cloned().collect();
        for id in &ids {
            self.stop_voices_for(id, tween);
        }
        self.stop_preview();
        ids
    }

    /// Reproduce `start_ms..end_ms` de un buffer estéreo 48kHz ya en memoria,
    /// por el mismo device que los pads — es la única forma de que el
    /// usuario juzgue un recorte como sonará al aire (PLAN.md §2, SPEC-recorte-v2.md).
    /// Corta cualquier preview anterior antes de arrancar el nuevo.
    ///
    /// El recorte se hace en Rust con índices enteros ya clampeados
    /// (`trim::clamp_ms_range_to_frames`), NO con la conversión ms→muestras
    /// en punto flotante de `StaticSoundData::slice()` — un ms en el borde
    /// exacto del clip podía redondear a un frame más allá del buffer y
    /// tronar el proceso entero (un panic de Rust no puede cruzar el límite
    /// del motor de audio en tiempo real, así que aborta en vez de fallar
    /// ese comando nomás).
    pub fn preview(&mut self, stereo_48k: &[f32], start_ms: u32, end_ms: u32) -> Result<(), EngineError> {
        self.stop_preview();
        let frame_count = stereo_48k.len() / 2;
        let (start_frame, end_frame) =
            super::trim::clamp_ms_range_to_frames(start_ms, end_ms, frame_count, TARGET_SAMPLE_RATE);

        let frames = stereo_to_frames(&stereo_48k[start_frame * 2..end_frame * 2]);
        let data = StaticSoundData {
            sample_rate: TARGET_SAMPLE_RATE,
            frames: Arc::from(frames),
            settings: StaticSoundSettings::default(),
            slice: None,
        };
        let handle = self.manager.play(data).map_err(|e| EngineError::Backend(e.to_string()))?;
        self.preview = Some(handle);
        Ok(())
    }

    /// Corta el preview activo, si hay uno. No-op si no hay nada sonando.
    pub fn stop_preview(&mut self) {
        if let Some(mut handle) = self.preview.take() {
            handle.stop(Tween::default());
        }
    }

    /// Lee posición/estado de las voces activas para los eventos `pad-tick` /
    /// `pad-stopped` (PLAN.md §6.6). Limpia las voces que ya terminaron solas.
    pub fn poll(&mut self) -> (Vec<PadTick>, Vec<String>) {
        let mut ticks = Vec::new();
        let mut fully_stopped = Vec::new();
        let pad_ids: Vec<String> = self.active.keys().cloned().collect();

        for pad_id in pad_ids {
            if let Some(voices) = self.active.get_mut(&pad_id) {
                voices.retain(|(h, _)| !matches!(h.state(), PlaybackState::Stopped));
                if voices.is_empty() {
                    fully_stopped.push(pad_id.clone());
                } else if let Some((handle, duration_ms)) = voices.last() {
                    let remaining_ms = *duration_ms as f64 - handle.position() * 1000.0;
                    ticks.push(PadTick { pad_id: pad_id.clone(), remaining_ms: remaining_ms.round() as i64 });
                }
            }
        }
        self.active.retain(|_, v| !v.is_empty());
        (ticks, fully_stopped)
    }

    fn stop_voices_for(&mut self, pad_id: &str, tween: Tween) {
        if let Some(voices) = self.active.get_mut(pad_id) {
            for (handle, _) in voices.iter_mut() {
                handle.stop(tween);
            }
        }
        self.active.remove(pad_id);
    }

    fn start_voice(&mut self, pad: &Pad) -> Result<(), EngineError> {
        let base = self.preloaded.get(&pad.id).ok_or_else(|| EngineError::NotPreloaded(pad.id.clone()))?;
        let mut data = base.clone();

        let mut settings = StaticSoundSettings::default()
            .volume(Decibels(pad.trim_db))
            .fade_in_tween(tween_ms(pad.fade_in_ms));
        if pad.loop_enabled {
            settings = settings.loop_region(..);
        }
        data.settings = settings;

        let handle = self.manager.play(data).map_err(|e| EngineError::Backend(e.to_string()))?;
        self.active.entry(pad.id.clone()).or_default().push((handle, pad.duration_ms));
        Ok(())
    }
}

fn tween_ms(ms: u32) -> Tween {
    Tween { duration: Duration::from_millis(ms as u64), ..Default::default() }
}

/// Convierte un buffer estéreo intercalado (como los que produce
/// `normalize::import_pipeline`) a los `Frame` que espera kira. Pura —
/// separa la conversión de datos de la reproducción en sí, que sí necesita
/// hardware (cpal). Usada por el preview del editor de recorte (SPEC-recorte-v2.md).
fn stereo_to_frames(stereo: &[f32]) -> Vec<Frame> {
    stereo.chunks_exact(2).map(|frame| Frame::new(frame[0], frame[1])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_to_frames_pairs_left_and_right_samples() {
        let stereo = vec![0.1, -0.1, 0.2, -0.2, 0.3, -0.3];

        let frames = stereo_to_frames(&stereo);

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], Frame::new(0.1, -0.1));
        assert_eq!(frames[1], Frame::new(0.2, -0.2));
        assert_eq!(frames[2], Frame::new(0.3, -0.3));
    }

    #[test]
    fn stereo_to_frames_of_empty_buffer_is_empty() {
        let frames = stereo_to_frames(&[]);
        assert!(frames.is_empty());
    }
}

fn negotiated_stream_config(device: &cpal::Device) -> cpal::StreamConfig {
    let ranges: Vec<SupportedRange> = device
        .supported_output_configs()
        .map(|cfgs| {
            cfgs.map(|c| {
                let (min_buffer_frames, max_buffer_frames) = match c.buffer_size() {
                    cpal::SupportedBufferSize::Range { min, max } => (*min, *max),
                    cpal::SupportedBufferSize::Unknown => (TARGET_BUFFER_FRAMES, TARGET_BUFFER_FRAMES),
                };
                SupportedRange {
                    min_sample_rate: c.min_sample_rate(),
                    max_sample_rate: c.max_sample_rate(),
                    min_buffer_frames,
                    max_buffer_frames,
                }
            })
            .collect()
        })
        .unwrap_or_default();

    let choice = if ranges.is_empty() {
        ConfigChoice {
            sample_rate: TARGET_SAMPLE_RATE,
            buffer_frames: TARGET_BUFFER_FRAMES,
            matches_target: false,
        }
    } else {
        negotiate_config(&ranges, TARGET_SAMPLE_RATE, TARGET_BUFFER_FRAMES)
    };

    cpal::StreamConfig {
        channels: 2,
        sample_rate: choice.sample_rate,
        buffer_size: cpal::BufferSize::Fixed(choice.buffer_frames),
    }
}

fn test_tone(freq_hz: f32, duration_secs: f32, sample_rate: u32) -> StaticSoundData {
    let n = (duration_secs * sample_rate as f32) as usize;
    let frames: Vec<Frame> = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let s = (t * freq_hz * std::f32::consts::TAU).sin() * 0.2;
            Frame::new(s, s)
        })
        .collect();

    StaticSoundData {
        sample_rate,
        frames: Arc::from(frames),
        settings: StaticSoundSettings::default(),
        slice: None,
    }
}
