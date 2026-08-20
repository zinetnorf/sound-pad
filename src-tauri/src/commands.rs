use std::sync::Mutex;

use tauri::{Emitter, Manager};

use crate::audio::devices::{self, DeviceInfo};
use crate::audio::engine::{AudioEngine, PlayOutcome};
use crate::audio::normalize;
use crate::models::{AppConfig, Bank, Pad};
use crate::storage;

/// Motor de audio persistente compartido por toda la sesión de la app.
/// `None` hasta que `init_engine` logra abrir un dispositivo de salida.
pub struct EngineState(pub Mutex<Option<AudioEngine>>);

/// Buffer decodificado + resampleado del archivo que el editor de recorte
/// tiene abierto ahora mismo. Poblado por `analyze_source`, consultado por
/// `snap_region`/`preview_region`/`import_pad`, liberado por
/// `clear_source_cache` al cerrar el modal. Evita decodificar el mismo MP3
/// tres veces (SPEC-recorte-v2.md "Forma de onda").
pub struct SourceCache {
    pub source_path: String,
    pub stereo_48k: Vec<f32>,
    pub source_sample_rate: u32,
    pub source_channels: u8,
}
pub struct SourceCacheState(pub Mutex<Option<SourceCache>>);

fn app_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub(crate) fn config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app_dir(app)?.join("config.json"))
}

/// Genera y persiste el banco "Default" de fábrica en la primera instalación
/// (cuando `config.json` todavía no existe). No aborta el arranque si falla
/// (recurso empaquetado faltante, etc.) — la app sigue arrancando vacía,
/// igual que si el usuario hubiera partido de cero manualmente.
fn seed_default_bank(app: &tauri::AppHandle, cfg_path: &std::path::Path) {
    let audio_dir = match app_dir(app).map(|d| d.join("audio")) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("seed de banco Default falló: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&audio_dir) {
        eprintln!("seed de banco Default falló: {e}");
        return;
    }

    match crate::seed::seed_default_config(app, &audio_dir) {
        Ok(config) => {
            if let Err(e) = storage::save(cfg_path, &config) {
                eprintln!("guardar config semilla falló: {e}");
            }
        }
        Err(e) => eprintln!("seed de banco Default falló: {e}"),
    }
}

/// Abre el dispositivo configurado (con fallback, PLAN.md §6.1), precarga el
/// banco activo y arranca el hilo de `pad-tick`. Se llama una vez al iniciar la app.
/// El estado se registra siempre, aunque no haya dispositivo disponible, para
/// que `trigger_pad`/`stop_pad`/`panic` puedan fallar con un error claro en vez
/// de que Tauri no encuentre el estado gestionado.
pub fn init_engine(app: &tauri::AppHandle) -> Result<(), String> {
    app.manage(EngineState(Mutex::new(None)));
    spawn_tick_thread(app.clone());

    let cfg_path = config_path(app)?;
    if !cfg_path.exists() {
        seed_default_bank(app, &cfg_path);
    }
    let config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;

    let available = devices::enumerate();
    if available.is_empty() {
        return Err("no hay dispositivos de salida disponibles".to_string());
    }
    let default_id = devices::default_device_id().unwrap_or_default();
    let (resolved, _kind) =
        devices::resolve_device(&available, &config.output_device.device_id, &config.output_device.label, &default_id);
    let device = devices::find_by_id(&resolved.id).ok_or("dispositivo resuelto no encontrado")?;

    let mut engine = AudioEngine::new(device).map_err(|e| e.to_string())?;
    if let Some(bank) = config.banks.iter().find(|b| b.id == config.active_bank_id) {
        let audio_dir = app_dir(app)?.join("audio");
        if let Err(e) = engine.preload(&bank.pads, &audio_dir) {
            eprintln!("preload del banco activo falló: {e}");
        }
    }

    let state = app.state::<EngineState>();
    *state.0.lock().map_err(|_| "motor no disponible".to_string())? = Some(engine);

    if let Err(e) = crate::hotkeys::refresh(app, &config) {
        eprintln!("refresh de atajos globales falló: {e}");
    }
    Ok(())
}

/// Registra el estado MIDI y, si hay un dispositivo persistido, se conecta.
/// Igual que `init_engine`: no tirar la app abajo si el dispositivo no aparece.
pub fn init_midi(app: &tauri::AppHandle) -> Result<(), String> {
    app.manage(crate::midi::MidiState::default());

    let cfg_path = config_path(app)?;
    let config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    let Some(device_name) = config.midi_device_name else { return Ok(()) };

    let state = app.state::<crate::midi::MidiState>();
    let conn = crate::midi::connect(&device_name, app.clone(), state.learning.clone())?;
    *state.connection.lock().map_err(|_| "midi no disponible".to_string())? = Some(conn);
    Ok(())
}

#[tauri::command]
pub fn list_midi_devices() -> Vec<String> {
    crate::midi::list_devices()
}

/// Cambia (o desconecta, con `device_name: null`) el dispositivo MIDI activo
/// y lo persiste (PLAN.md §7.7).
#[tauri::command]
pub fn set_midi_device(
    app: tauri::AppHandle,
    state: tauri::State<crate::midi::MidiState>,
    device_name: Option<String>,
) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    config.midi_device_name = device_name.clone();
    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;

    let mut guard = state.connection.lock().map_err(|_| "midi no disponible".to_string())?;
    *guard = None; // corta la conexión anterior antes de abrir la nueva
    if let Some(name) = device_name {
        *guard = Some(crate::midi::connect(&name, app.clone(), state.learning.clone())?);
    }
    Ok(())
}

/// Activa el modo captura: la próxima nota que llegue se emite como
/// `midi-learn-captured` en vez de disparar un pad (PLAN.md §7.6).
#[tauri::command]
pub fn start_midi_learn(state: tauri::State<crate::midi::MidiState>) -> Result<(), String> {
    state.learning.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn cancel_midi_learn(state: tauri::State<crate::midi::MidiState>) -> Result<(), String> {
    state.learning.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

fn spawn_tick_thread(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Cada ~2s (20 vueltas de 100ms) se revisa que el dispositivo de
        // salida configurado siga existiendo — PLAN.md §6.1.2. No hace falta
        // un hilo aparte: el tick ya corre cada 100ms, esto solo se apura menos.
        let mut device_check_counter: u32 = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));

            device_check_counter += 1;
            if device_check_counter >= 20 {
                device_check_counter = 0;
                check_output_device(&app);
            }

            let Some(state) = app.try_state::<EngineState>() else { continue };
            let Ok(mut guard) = state.0.lock() else { continue };
            let Some(engine) = guard.as_mut() else { continue };
            let (ticks, stopped) = engine.poll();
            drop(guard);

            for t in ticks {
                let _ = app.emit("pad-tick", serde_json::json!({ "padId": t.pad_id, "remainingMs": t.remaining_ms }));
            }
            for pad_id in stopped {
                let _ = app.emit("pad-stopped", serde_json::json!({ "padId": pad_id }));
            }
        }
    });
}

/// Si el dispositivo de salida configurado ya no está entre los disponibles,
/// cae al default, avisa a la UI y persiste el cambio — nunca se queda muda
/// sin explicación (PLAN.md §6.1.2 y criterio de aceptación §14).
fn check_output_device(app: &tauri::AppHandle) {
    let Ok(cfg_path) = config_path(app) else { return };
    let Ok(mut config) = storage::load_or_default(&cfg_path) else { return };
    if config.output_device.device_id.is_empty() {
        return;
    }

    let available = devices::enumerate();
    if available.iter().any(|d| d.id == config.output_device.device_id) {
        return;
    }

    let attempted_label = config.output_device.label.clone();
    let Some(default_id) = devices::default_device_id() else { return };
    let Some(device) = devices::find_by_id(&default_id) else { return };
    let Ok(mut engine) = AudioEngine::new(device) else { return };

    if let (Some(bank), Ok(dir)) = (config.banks.iter().find(|b| b.id == config.active_bank_id), app_dir(app)) {
        let _ = engine.preload(&bank.pads, &dir.join("audio"));
    }

    if let Some(fallback) = available.into_iter().find(|d| d.id == default_id) {
        config.output_device = crate::models::OutputDeviceRef { device_id: fallback.id, label: fallback.label };
    }
    let _ = storage::save(&cfg_path, &config);

    if let Some(state) = app.try_state::<EngineState>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(engine);
        }
    }

    let _ = app.emit("output-device-lost", serde_json::json!({ "attemptedLabel": attempted_label }));
}

/// Re-precarga el banco activo en el motor en vivo tras un cambio de config
/// (importar, editar, eliminar, reordenar), para no requerir reiniciar la app.
fn refresh_engine_cache(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let Some(state) = app.try_state::<EngineState>() else { return Ok(()) };
    let Some(bank) = config.banks.iter().find(|b| b.id == config.active_bank_id) else { return Ok(()) };
    let audio_dir = app_dir(app)?.join("audio");

    let mut guard = state.0.lock().map_err(|_| "motor no disponible".to_string())?;
    if let Some(engine) = guard.as_mut() {
        engine.preload(&bank.pads, &audio_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Re-sincroniza todo lo que vive "en caliente" tras un cambio de config:
/// la precarga del motor y los atajos globales (PLAN.md §8). Un atajo global
/// que falla no debe tumbar el guardado del pad, así que solo se loguea.
fn refresh_live_state(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    refresh_engine_cache(app, config)?;
    if let Err(e) = crate::hotkeys::refresh(app, config) {
        eprintln!("refresh de atajos globales falló: {e}");
    }
    Ok(())
}

#[tauri::command]
pub fn get_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    let path = config_path(&app)?;
    storage::load_or_default(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    let path = config_path(&app)?;
    storage::save(&path, &config).map_err(|e| e.to_string())?;
    // Setter genérico de Configuración (§7.7): atajos on/off, modificador,
    // hotkey de pánico... cualquier cambio ahí necesita re-sincronizar.
    refresh_live_state(&app, &config)
}

#[tauri::command]
pub fn list_output_devices() -> Vec<DeviceInfo> {
    devices::enumerate()
}

/// Corre el pipeline de normalización (PLAN.md §6.3), guarda el WAV resultante
/// en `audio/{pad_uuid}.wav` y agrega el pad al banco activo (creándolo si es
/// el primer pad de la app). Fase 3: modal de agregar pad de punta a punta.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // firma plana 1:1 con el payload de invoke() del frontend
pub async fn import_pad(
    app: tauri::AppHandle,
    source_path: String,
    name: String,
    color: String,
    hotkey: Option<String>,
    midi_note: Option<u8>,
    trim_start_ms: Option<u32>,
    trim_end_ms: Option<u32>,
) -> Result<Pad, String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;

    let trim = match (trim_start_ms, trim_end_ms) {
        (Some(start_ms), Some(end_ms)) => Some(crate::audio::trim::TrimRegion { start_ms, end_ms }),
        _ => None,
    };

    // Si el editor de recorte ya decodificó y cacheó este mismo archivo, se
    // reusa el buffer en vez de decodificar el MP3 una tercera vez.
    let cached = app.try_state::<SourceCacheState>().and_then(|s| {
        s.0.lock().ok().and_then(|guard| guard.as_ref().filter(|c| c.source_path == source_path).map(|c| (c.stereo_48k.clone(), c.source_sample_rate, c.source_channels)))
    });

    // Generado antes del trabajo pesado para poder escribir el WAV en el
    // mismo hilo bloqueante, sin un segundo salto de ida y vuelta.
    let pad_id = uuid::Uuid::new_v4().to_string();
    let audio_dir = app_dir(&app)?.join("audio");
    std::fs::create_dir_all(&audio_dir).map_err(|e| e.to_string())?;
    let wav_path = audio_dir.join(format!("{pad_id}.wav"));

    let target_lufs = config.target_lufs;
    let source_path_for_decode = source_path.clone();
    // Decodificar, resamplear, medir LUFS y escribir el WAV son CPU/IO
    // pesados (varios segundos en un MP3 de un minuto) — corren en el pool
    // de hilos bloqueantes de Tauri para no congelar el WebView, que de
    // otro modo se queda sin poder pintar ni el texto de "normalizando..."
    // mientras el comando corre en el mismo hilo que despacha el IPC.
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = match cached {
            Some((stereo, source_sample_rate, source_channels)) => {
                normalize::normalize_stereo(stereo, source_sample_rate, source_channels, target_lufs, trim)?
            }
            None => normalize::import_pipeline(std::path::Path::new(&source_path_for_decode), target_lufs, trim)?,
        };
        normalize::write_wav(&wav_path, &result.stereo_48k)?;
        Ok::<_, normalize::NormalizeError>(result)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let duration_ms = (result.stereo_48k.len() / 2) as f64 / normalize::TARGET_SAMPLE_RATE as f64 * 1000.0;
    let original_filename = std::path::Path::new(&source_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    if config.banks.is_empty() {
        let bank = Bank { id: uuid::Uuid::new_v4().to_string(), name: "Banco 1".to_string(), order: 0, pads: Vec::new() };
        config.active_bank_id = bank.id.clone();
        config.banks.push(bank);
    }
    let bank = config
        .banks
        .iter_mut()
        .find(|b| b.id == config.active_bank_id)
        .ok_or_else(|| "banco activo no encontrado".to_string())?;

    let pad = Pad {
        id: pad_id,
        name,
        color,
        hotkey,
        midi_note,
        order: bank.pads.len() as u32,
        duration_ms: duration_ms.round() as u32,
        original_filename,
        source_sample_rate: result.source_sample_rate,
        source_channels: result.source_channels,
        measured_lufs: result.measured_lufs,
        applied_gain_db: result.applied_gain_db,
        gain_capped: result.gain_capped,
        ..Pad::default()
    };
    bank.validate_hotkey(&pad)?;
    bank.validate_midi_note(&pad)?;

    bank.pads.push(pad.clone());
    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)?;

    Ok(pad)
}

/// Cuántos buckets de picos min/max se calculan para la forma de onda del
/// editor de recorte — no se manda la muestra cruda al WebView, un minuto a
/// 48kHz estéreo son ~5.7M valores (SPEC-recorte-v2.md "Forma de onda").
const WAVEFORM_BUCKETS: usize = 2000;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAnalysis {
    pub duration_ms: u32,
    /// Un `Vec` por canal (L, R); cada uno el pico con signo de mayor
    /// magnitud por bucket — wavesurfer.js dibuja un array plano por canal,
    /// no pares min/max intercalados.
    pub peaks: Vec<Vec<f32>>,
    pub source_sample_rate: u32,
    pub source_channels: u8,
}

/// Decodifica y resamplea el archivo elegido en el editor de "Agregar Pad",
/// calcula los picos para la forma de onda y cachea el buffer en memoria
/// para que `snap_region`/`preview_region`/`import_pad` no vuelvan a
/// decodificarlo (SPEC-recorte-v2.md).
#[tauri::command]
pub async fn analyze_source(app: tauri::AppHandle, source_path: String) -> Result<SourceAnalysis, String> {
    let path_for_decode = source_path.clone();
    // Decodificar un MP3 de un minuto + resamplear a alta calidad + calcular
    // 2000 picos toma varios segundos — igual que en `import_pad`, corre en
    // el pool de hilos bloqueantes para que el WebView pueda seguir pintando
    // "Cargando forma de onda..." en vez de parecer colgado.
    let (stereo, source_sample_rate, source_channels, peaks) = tauri::async_runtime::spawn_blocking(move || {
        let decoded = normalize::decode_file(std::path::Path::new(&path_for_decode))?;
        let resampled = normalize::resample_to_48k(&decoded.samples, decoded.channels, decoded.sample_rate)?;
        let stereo = normalize::to_stereo(&resampled, decoded.channels);
        let peaks = crate::audio::trim::compute_peaks(&stereo, WAVEFORM_BUCKETS);
        Ok::<_, normalize::NormalizeError>((stereo, decoded.sample_rate, decoded.channels, peaks))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let duration_ms = (stereo.len() / 2) as f64 / normalize::TARGET_SAMPLE_RATE as f64 * 1000.0;

    let state = app.state::<SourceCacheState>();
    let mut guard = state.0.lock().map_err(|_| "cache de análisis no disponible".to_string())?;
    *guard = Some(SourceCache { source_path, stereo_48k: stereo, source_sample_rate, source_channels });

    Ok(SourceAnalysis {
        duration_ms: duration_ms.round() as u32,
        peaks,
        source_sample_rate,
        source_channels,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnappedRegion {
    pub start_ms: u32,
    pub end_ms: u32,
}

/// Snapea ambos bordes de la región al cruce por cero más cercano contra el
/// buffer cacheado por `analyze_source` — el frontend actualiza los campos
/// numéricos con este valor al soltar cada handle (SPEC-recorte-v2.md "Snap
/// a cruce por cero"). El mismo snap se vuelve a aplicar en `import_pad`
/// (idempotente), así que lo que el usuario ve es lo que obtiene.
#[tauri::command]
pub fn snap_region(state: tauri::State<SourceCacheState>, start_ms: u32, end_ms: u32) -> Result<SnappedRegion, String> {
    let guard = state.0.lock().map_err(|_| "cache de análisis no disponible".to_string())?;
    let cache = guard.as_ref().ok_or("no hay un archivo analizado — llamar a analyze_source primero")?;

    let ms_to_frame = |ms: u32| (ms as u64 * normalize::TARGET_SAMPLE_RATE as u64 / 1000) as usize;
    let frame_to_ms = |frame: usize| (frame as u64 * 1000 / normalize::TARGET_SAMPLE_RATE as u64) as u32;

    let start = crate::audio::trim::snap_to_zero_crossing(&cache.stereo_48k, ms_to_frame(start_ms), crate::audio::trim::ZERO_CROSSING_SEARCH_FRAMES);
    let end = crate::audio::trim::snap_to_zero_crossing(&cache.stereo_48k, ms_to_frame(end_ms), crate::audio::trim::ZERO_CROSSING_SEARCH_FRAMES);

    Ok(SnappedRegion { start_ms: frame_to_ms(start), end_ms: frame_to_ms(end) })
}

/// Reproduce la región elegida por el mismo device que los pads — nunca por
/// el device por defecto del sistema — para que el usuario juzgue el
/// recorte tal como sonará al aire (PLAN.md §2, SPEC-recorte-v2.md).
#[tauri::command]
pub fn preview_region(engine_state: tauri::State<EngineState>, cache_state: tauri::State<SourceCacheState>, start_ms: u32, end_ms: u32) -> Result<(), String> {
    let cache_guard = cache_state.0.lock().map_err(|_| "cache de análisis no disponible".to_string())?;
    let cache = cache_guard.as_ref().ok_or("no hay un archivo analizado — llamar a analyze_source primero")?;

    let mut engine_guard = engine_state.0.lock().map_err(|_| "motor no disponible".to_string())?;
    let engine = engine_guard.as_mut().ok_or("motor de audio no inicializado")?;
    engine.preview(&cache.stereo_48k, start_ms, end_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_preview(state: tauri::State<EngineState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|_| "motor no disponible".to_string())?;
    if let Some(engine) = guard.as_mut() {
        engine.stop_preview();
    }
    Ok(())
}

/// Libera el buffer cacheado por `analyze_source` — llamado al cerrar o
/// cancelar el modal de "Agregar Pad" para no dejar un minuto de audio
/// decodificado en memoria indefinidamente.
#[tauri::command]
pub fn clear_source_cache(state: tauri::State<SourceCacheState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|_| "cache de análisis no disponible".to_string())?;
    *guard = None;
    Ok(())
}

/// Reemplaza un pad completo (trim, loop, nombre, color, re-disparo, fades...)
/// dentro de su banco y persiste. Usado por los sliders inline y el modal de editar.
#[tauri::command]
pub fn update_pad(app: tauri::AppHandle, bank_id: String, pad: Pad) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;

    let bank = config.banks.iter_mut().find(|b| b.id == bank_id).ok_or("banco no encontrado")?;
    bank.validate_hotkey(&pad)?;
    bank.validate_midi_note(&pad)?;
    if !bank.replace_pad(pad) {
        return Err("pad no encontrado".to_string());
    }

    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)
}

/// Quita el pad de su banco y borra su archivo de audio del disco
/// (PLAN.md §7.6 — la eliminación solo es accesible desde el modal de editar).
fn delete_pad_impl(app_dir: &std::path::Path, config: &mut AppConfig, bank_id: &str, pad_id: &str) -> Result<(), String> {
    let bank = config.banks.iter_mut().find(|b| b.id == bank_id).ok_or("banco no encontrado")?;
    let removed = bank.remove_pad(pad_id).ok_or("pad no encontrado")?;

    let wav_path = app_dir.join("audio").join(format!("{}.wav", removed.id));
    if wav_path.exists() {
        std::fs::remove_file(&wav_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_pad(app: tauri::AppHandle, bank_id: String, pad_id: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    let dir = app_dir(&app)?;

    delete_pad_impl(&dir, &mut config, &bank_id, &pad_id)?;
    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;

    if let Some(state) = app.try_state::<EngineState>() {
        if let Ok(mut guard) = state.0.lock() {
            if let Some(engine) = guard.as_mut() {
                engine.stop(&pad_id, 50);
            }
        }
    }
    refresh_live_state(&app, &config)
}

/// Reordena los pads de un banco tras un drag & drop (PLAN.md §7.2).
#[tauri::command]
pub fn reorder_pads(app: tauri::AppHandle, bank_id: String, ordered_pad_ids: Vec<String>) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;

    let bank = config.banks.iter_mut().find(|b| b.id == bank_id).ok_or("banco no encontrado")?;
    bank.reorder_pads(&ordered_pad_ids);

    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)
}

/// Crea un banco vacío y lo vuelve el activo (PLAN.md §10) — el flujo natural
/// tras crear uno es empezar a agregarle pads de inmediato.
#[tauri::command]
pub fn create_bank(app: tauri::AppHandle, name: String) -> Result<Bank, String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;

    let bank = Bank { id: uuid::Uuid::new_v4().to_string(), name, order: config.banks.len() as u32, pads: Vec::new() };
    config.banks.push(bank.clone());
    config.active_bank_id = bank.id.clone();

    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)?;
    Ok(bank)
}

#[tauri::command]
pub fn rename_bank(app: tauri::AppHandle, bank_id: String, name: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;

    let bank = config.banks.iter_mut().find(|b| b.id == bank_id).ok_or("banco no encontrado")?;
    bank.name = name;

    storage::save(&cfg_path, &config).map_err(|e| e.to_string())
}

/// Quita el banco y borra el audio de todos sus pads. Si era el banco activo,
/// pasa al primero que quede (o a ninguno si era el último).
fn delete_bank_impl(audio_dir: &std::path::Path, config: &mut AppConfig, bank_id: &str) -> Result<(), String> {
    let index = config.banks.iter().position(|b| b.id == bank_id).ok_or("banco no encontrado")?;
    let removed = config.banks.remove(index);

    for pad in &removed.pads {
        let wav_path = audio_dir.join(format!("{}.wav", pad.id));
        if wav_path.exists() {
            std::fs::remove_file(&wav_path).map_err(|e| e.to_string())?;
        }
    }

    if config.active_bank_id == bank_id {
        config.active_bank_id = config.banks.first().map(|b| b.id.clone()).unwrap_or_default();
    }
    Ok(())
}

#[tauri::command]
pub fn delete_bank(app: tauri::AppHandle, bank_id: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    let dir = app_dir(&app)?.join("audio");

    delete_bank_impl(&dir, &mut config, &bank_id)?;
    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)
}

/// Reordena las tabs de bancos tras un drag & drop (PLAN.md §10).
#[tauri::command]
pub fn reorder_banks(app: tauri::AppHandle, ordered_bank_ids: Vec<String>) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    config.reorder_banks(&ordered_bank_ids);
    storage::save(&cfg_path, &config).map_err(|e| e.to_string())
}

/// Cambia de banco activo: detiene todo lo que suene y precarga el banco
/// nuevo (PLAN.md §10) — nunca queda sonando algo del banco anterior.
#[tauri::command]
pub fn set_active_bank(app: tauri::AppHandle, bank_id: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    if !config.banks.iter().any(|b| b.id == bank_id) {
        return Err("banco no encontrado".to_string());
    }

    if let Some(state) = app.try_state::<EngineState>() {
        let stopped = {
            let mut guard = state.0.lock().map_err(|_| "motor no disponible".to_string())?;
            match guard.as_mut() {
                Some(engine) => engine.stop_all(50),
                None => Vec::new(),
            }
        };
        for pad_id in stopped {
            let _ = app.emit("pad-stopped", serde_json::json!({ "padId": pad_id }));
        }
    }

    config.active_bank_id = bank_id;
    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)
}

/// Cambia el dispositivo de salida sin reiniciar la app (PLAN.md §6.1):
/// reconstruye el motor sobre el nuevo dispositivo y precarga el banco activo.
#[tauri::command]
pub fn set_output_device(app: tauri::AppHandle, device_id: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;

    let available = devices::enumerate();
    let target = available.iter().find(|d| d.id == device_id).ok_or("dispositivo no encontrado")?;
    config.output_device = crate::models::OutputDeviceRef { device_id: target.id.clone(), label: target.label.clone() };

    let device = devices::find_by_id(&device_id).ok_or("dispositivo no encontrado")?;
    let mut engine = AudioEngine::new(device).map_err(|e| e.to_string())?;
    if let Some(bank) = config.banks.iter().find(|b| b.id == config.active_bank_id) {
        let audio_dir = app_dir(&app)?.join("audio");
        if let Err(e) = engine.preload(&bank.pads, &audio_dir) {
            eprintln!("preload tras cambiar de dispositivo falló: {e}");
        }
    }

    let state = app.state::<EngineState>();
    *state.0.lock().map_err(|_| "motor no disponible".to_string())? = Some(engine);

    storage::save(&cfg_path, &config).map_err(|e| e.to_string())
}

/// Exporta `config.json` + `audio/` a un `.zip` en la ruta elegida por el
/// usuario (PLAN.md §11).
#[tauri::command]
pub fn export_config(app: tauri::AppHandle, dest_path: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let audio_dir = app_dir(&app)?.join("audio");
    crate::export::export_zip(&cfg_path, &audio_dir, std::path::Path::new(&dest_path)).map_err(|e| e.to_string())
}

/// Restaura `config.json` + `audio/` desde un `.zip` (PLAN.md §11) y
/// re-sincroniza el motor y los atajos con lo recién importado.
#[tauri::command]
pub fn import_config(app: tauri::AppHandle, source_path: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let audio_dir = app_dir(&app)?.join("audio");
    crate::export::import_zip(std::path::Path::new(&source_path), &cfg_path, &audio_dir).map_err(|e| e.to_string())?;

    let config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)
}

/// Recalcula la ganancia de todos los pads existentes hacia un nuevo
/// `target_lufs` (botón "Renormalizar todo", PLAN.md §6.3). Un pad que falle
/// no aborta el resto — se loguea y se sigue con los demás.
#[tauri::command]
pub fn renormalize_all(app: tauri::AppHandle, target_lufs: f32) -> Result<AppConfig, String> {
    let cfg_path = config_path(&app)?;
    let mut config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    config.target_lufs = target_lufs;

    let audio_dir = app_dir(&app)?.join("audio");
    for bank in config.banks.iter_mut() {
        for pad in bank.pads.iter_mut() {
            let wav_path = audio_dir.join(format!("{}.wav", pad.id));
            match normalize::renormalize_file(&wav_path, target_lufs) {
                Ok(r) => {
                    pad.measured_lufs = r.measured_lufs;
                    pad.applied_gain_db = r.applied_gain_db;
                    pad.gain_capped = r.gain_capped;
                }
                Err(e) => eprintln!("renormalizar \"{}\" falló: {e}", pad.name),
            }
        }
    }

    storage::save(&cfg_path, &config).map_err(|e| e.to_string())?;
    refresh_live_state(&app, &config)?;
    Ok(config)
}

fn find_pad<'a>(config: &'a AppConfig, bank_id: &str, pad_id: &str) -> Result<&'a Pad, String> {
    config
        .banks
        .iter()
        .find(|b| b.id == bank_id)
        .ok_or("banco no encontrado")?
        .pads
        .iter()
        .find(|p| p.id == pad_id)
        .ok_or_else(|| "pad no encontrado".to_string())
}

/// Dispara un pad respetando su modo de re-disparo (PLAN.md §6.5). Compartida
/// por el comando `trigger_pad` (clic/tecla suelta) y por los atajos globales
/// (PLAN.md §8) — ambos caminos terminan en el mismo lugar.
pub fn trigger(app: &tauri::AppHandle, bank_id: &str, pad_id: &str) -> Result<(), String> {
    let cfg_path = config_path(app)?;
    let config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    let pad = find_pad(&config, bank_id, pad_id)?;

    let state = app.state::<EngineState>();
    let mut guard = state.0.lock().map_err(|_| "motor no disponible".to_string())?;
    let engine = guard.as_mut().ok_or("motor de audio no inicializado")?;
    let outcome = engine.play(pad).map_err(|e| e.to_string())?;
    drop(guard);

    match outcome {
        PlayOutcome::Started => {
            let _ = app.emit("pad-started", serde_json::json!({ "padId": pad_id, "durationMs": pad.duration_ms }));
        }
        PlayOutcome::StoppedOnly => {
            let _ = app.emit("pad-stopped", serde_json::json!({ "padId": pad_id }));
        }
        PlayOutcome::Ignored => {}
    }
    Ok(())
}

/// Dispara un pad respetando su modo de re-disparo (PLAN.md §6.5). Fase 5.
#[tauri::command]
pub fn trigger_pad(app: tauri::AppHandle, bank_id: String, pad_id: String) -> Result<(), String> {
    trigger(&app, &bank_id, &pad_id)
}

/// Detiene un pad manualmente, con su fade-out configurado (PLAN.md §6.5).
#[tauri::command]
pub fn stop_pad(app: tauri::AppHandle, state: tauri::State<EngineState>, bank_id: String, pad_id: String) -> Result<(), String> {
    let cfg_path = config_path(&app)?;
    let config = storage::load_or_default(&cfg_path).map_err(|e| e.to_string())?;
    let pad = find_pad(&config, &bank_id, &pad_id)?;
    let fade_out_ms = pad.fade_out_ms;

    let mut guard = state.0.lock().map_err(|_| "motor no disponible".to_string())?;
    if let Some(engine) = guard.as_mut() {
        engine.stop(&pad_id, fade_out_ms);
    }
    drop(guard);

    let _ = app.emit("pad-stopped", serde_json::json!({ "padId": pad_id }));
    Ok(())
}

/// Botón de pánico: detiene todo, de inmediato, con un fade-out corto para
/// evitar el chasquido (PLAN.md §6.5 y §14).
#[tauri::command]
pub fn panic(app: tauri::AppHandle) -> Result<(), String> {
    panic_now(&app)
}

/// Compartida por el comando `panic` (clic) y el atajo global del botón de
/// pánico (PLAN.md §7.7) — igual que `trigger`, ambos caminos van al mismo lugar.
pub fn panic_now(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<EngineState>();
    let stopped = {
        let mut guard = state.0.lock().map_err(|_| "motor no disponible".to_string())?;
        match guard.as_mut() {
            Some(engine) => engine.stop_all(50),
            None => Vec::new(),
        }
    };
    for pad_id in stopped {
        let _ = app.emit("pad-stopped", serde_json::json!({ "padId": pad_id }));
    }
    Ok(())
}

/// Abre el dispositivo elegido y reproduce un tono de prueba.
/// Fase 1: valida que el audio sale por el dispositivo correcto (PLAN.md §13).
#[tauri::command]
pub fn play_test_tone(device_id: String) -> Result<(), String> {
    let device = devices::find_by_id(&device_id)
        .ok_or_else(|| format!("dispositivo no encontrado: {device_id}"))?;
    let mut engine = AudioEngine::new(device).map_err(|e| e.to_string())?;
    engine.play_test_tone().map_err(|e| e.to_string())?;
    // Mantiene el proceso vivo el tiempo del tono; el stream vive en AudioEngine.
    std::thread::sleep(std::time::Duration::from_millis(600));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Bank;

    fn config_with_one_pad(pad_id: &str) -> AppConfig {
        let pad = Pad { id: pad_id.to_string(), name: "Aplausos".to_string(), ..Pad::default() };
        AppConfig {
            active_bank_id: "b1".to_string(),
            banks: vec![Bank { id: "b1".to_string(), name: "Banco 1".to_string(), order: 0, pads: vec![pad] }],
            ..AppConfig::default()
        }
    }

    #[test]
    fn delete_pad_impl_removes_pad_and_deletes_its_wav_file() {
        let dir = tempfile::tempdir().unwrap();
        let audio_dir = dir.path().join("audio");
        std::fs::create_dir_all(&audio_dir).unwrap();
        let wav_path = audio_dir.join("pad-1.wav");
        std::fs::write(&wav_path, b"fake wav").unwrap();

        let mut config = config_with_one_pad("pad-1");

        delete_pad_impl(dir.path(), &mut config, "b1", "pad-1").unwrap();

        assert!(config.banks[0].pads.is_empty());
        assert!(!wav_path.exists());
    }

    #[test]
    fn delete_pad_impl_errors_when_pad_not_found_and_leaves_files_alone() {
        let dir = tempfile::tempdir().unwrap();
        let audio_dir = dir.path().join("audio");
        std::fs::create_dir_all(&audio_dir).unwrap();
        let wav_path = audio_dir.join("pad-1.wav");
        std::fs::write(&wav_path, b"fake wav").unwrap();

        let mut config = config_with_one_pad("pad-1");

        let result = delete_pad_impl(dir.path(), &mut config, "b1", "no-existe");

        assert!(result.is_err());
        assert_eq!(config.banks[0].pads.len(), 1);
        assert!(wav_path.exists());
    }

    #[test]
    fn delete_pad_impl_is_ok_when_the_wav_file_was_already_missing() {
        // Defensivo: no debe fallar si el archivo ya no está (borrado externo, etc).
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_one_pad("pad-1");

        let result = delete_pad_impl(dir.path(), &mut config, "b1", "pad-1");

        assert!(result.is_ok());
        assert!(config.banks[0].pads.is_empty());
    }

    fn config_with_two_banks() -> AppConfig {
        let pad_a = Pad { id: "pad-a".to_string(), name: "A".to_string(), ..Pad::default() };
        let pad_b = Pad { id: "pad-b".to_string(), name: "B".to_string(), ..Pad::default() };
        AppConfig {
            active_bank_id: "b1".to_string(),
            banks: vec![
                Bank { id: "b1".to_string(), name: "Banco 1".to_string(), order: 0, pads: vec![pad_a] },
                Bank { id: "b2".to_string(), name: "Banco 2".to_string(), order: 1, pads: vec![pad_b] },
            ],
            ..AppConfig::default()
        }
    }

    #[test]
    fn delete_bank_impl_removes_bank_and_deletes_all_its_pad_wav_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let wav_a = dir.path().join("pad-a.wav");
        std::fs::write(&wav_a, b"fake").unwrap();

        let mut config = config_with_two_banks();

        delete_bank_impl(dir.path(), &mut config, "b1").unwrap();

        assert_eq!(config.banks.len(), 1);
        assert_eq!(config.banks[0].id, "b2");
        assert!(!wav_a.exists());
    }

    #[test]
    fn delete_bank_impl_switches_active_bank_when_deleting_the_active_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_two_banks();

        delete_bank_impl(dir.path(), &mut config, "b1").unwrap();

        assert_eq!(config.active_bank_id, "b2");
    }

    #[test]
    fn delete_bank_impl_leaves_active_bank_untouched_when_deleting_a_different_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_two_banks();

        delete_bank_impl(dir.path(), &mut config, "b2").unwrap();

        assert_eq!(config.active_bank_id, "b1");
    }

    #[test]
    fn delete_bank_impl_clears_active_bank_id_when_deleting_the_last_bank() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_one_pad("pad-1");

        delete_bank_impl(dir.path(), &mut config, "b1").unwrap();

        assert!(config.banks.is_empty());
        assert_eq!(config.active_bank_id, "");
    }
}
