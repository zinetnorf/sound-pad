use ebur128::{EbuR128, Mode};
use thiserror::Error;

pub const TARGET_SAMPLE_RATE: u32 = 48_000;
pub const TRUE_PEAK_CEILING_DBTP: f32 = -1.0;
const SHORT_CLIP_THRESHOLD_SECS: f64 = 3.0;
const CHANNELS: u32 = 2;

#[derive(Debug, Error)]
pub enum NormalizeError {
    #[error("error de medición de loudness: {0}")]
    Loudness(#[from] ebur128::Error),
    #[error("no se pudo leer el archivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("no se pudo decodificar el audio: {0}")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("el archivo no tiene una pista de audio decodificable")]
    NoTrack,
    #[error("no se pudo determinar el sample rate del archivo original")]
    NoSampleRate,
    #[error("error al resamplear: {0}")]
    Resample(#[from] rubato::ResampleError),
    #[error("error al construir el resampler: {0}")]
    ResamplerConstruction(#[from] rubato::ResamplerConstructionError),
    #[error("error al escribir el WAV: {0}")]
    Wav(#[from] hound::Error),
    #[error("error al recortar el audio: {0}")]
    Trim(#[from] super::trim::TrimError),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessMeasurement {
    pub loudness_lufs: f32,
    pub true_peak_dbtp: f32,
}

/// Mide un buffer estéreo intercalado a `sample_rate`. Rama gated (>=3s) o
/// sin gating (<3s) según PLAN.md §6.3 paso 4 — la mayoría de los efectos
/// caen en la segunda rama y el gate de BS.1770 los mediría mal.
pub fn measure_loudness(stereo_interleaved: &[f32], sample_rate: u32) -> Result<LoudnessMeasurement, NormalizeError> {
    let frame_count = stereo_interleaved.len() / 2;
    let duration_secs = frame_count as f64 / sample_rate as f64;
    let duration_ms = (duration_secs * 1000.0).round() as u32;

    let mut meter = EbuR128::new(CHANNELS, sample_rate, Mode::I | Mode::S | Mode::TRUE_PEAK)?;
    meter.set_max_window(duration_ms.max(3000))?;
    meter.add_frames_f32(stereo_interleaved)?;

    let loudness_lufs = if duration_secs >= SHORT_CLIP_THRESHOLD_SECS {
        meter.loudness_global()?
    } else {
        meter.loudness_window(duration_ms.max(1))?
    } as f32;

    let true_peak_linear = (0..CHANNELS)
        .map(|ch| meter.true_peak(ch))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(f64::NEG_INFINITY, f64::max);
    let true_peak_dbtp = (20.0 * true_peak_linear.log10()) as f32;

    Ok(LoudnessMeasurement { loudness_lufs, true_peak_dbtp })
}

/// Reduce/expande a estéreo: mono se duplica a L/R, más de 2 canales
/// se recorta a los primeros dos. `samples` está intercalado. PLAN.md §6.3 paso 3.
pub fn to_stereo(samples: &[f32], channels: u8) -> Vec<f32> {
    match channels {
        1 => samples.iter().flat_map(|&s| [s, s]).collect(),
        n if n >= 2 => samples
            .chunks(n as usize)
            .flat_map(|frame| [frame[0], frame[1]])
            .collect(),
        _ => Vec::new(),
    }
}

/// Multiplica cada muestra por la ganancia lineal equivalente a `gain_db`.
pub fn apply_gain(samples: &mut [f32], gain_db: f32) {
    let factor = 10f32.powf(gain_db / 20.0);
    for s in samples.iter_mut() {
        *s *= factor;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainResult {
    pub gain_db: f32,
    pub capped: bool,
}

/// Ganancia para llegar a `target_lufs`, recortada si excede el techo de
/// true peak (PLAN.md §6.3 pasos 5-6: preferible un pad bajo a uno distorsionado).
pub fn compute_gain(measured_lufs: f32, target_lufs: f32, true_peak_dbtp: f32) -> GainResult {
    let gain_db = target_lufs - measured_lufs;
    let headroom_db = TRUE_PEAK_CEILING_DBTP - true_peak_dbtp;

    if gain_db > headroom_db {
        GainResult { gain_db: headroom_db, capped: true }
    } else {
        GainResult { gain_db, capped: false }
    }
}

pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u8,
    /// Intercalado, en el orden de canales original del archivo.
    pub samples: Vec<f32>,
}

/// Decodifica MP3 o WAV con symphonia. Impura — I/O de disco.
pub fn decode_file(path: &std::path::Path) -> Result<DecodedAudio, NormalizeError> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;

    let track = format.default_track().ok_or(NormalizeError::NoTrack)?.clone();
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.ok_or(NormalizeError::NoSampleRate)?;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2) as u8;

    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut samples: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(_)) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                buf.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buf.samples());
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(DecodedAudio { sample_rate, channels, samples })
}

/// Resamplea audio intercalado (con `channels` canales) a 48kHz usando
/// `rubato` (SincFixedIn, calidad alta — el proceso es offline). PLAN.md §6.3 paso 2.
pub fn resample_to_48k(samples: &[f32], channels: u8, source_rate: u32) -> Result<Vec<f32>, NormalizeError> {
    use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

    if source_rate == TARGET_SAMPLE_RATE || samples.is_empty() {
        return Ok(samples.to_vec());
    }

    let channels = channels as usize;
    let frame_count = samples.len() / channels;

    // De intercalado a planar (un Vec por canal), como pide rubato.
    let mut planar: Vec<Vec<f32>> = vec![Vec::with_capacity(frame_count); channels];
    for frame in samples.chunks(channels) {
        for (ch, &s) in frame.iter().enumerate() {
            planar[ch].push(s);
        }
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let ratio = TARGET_SAMPLE_RATE as f64 / source_rate as f64;
    let chunk_size = 1024;
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, channels)?;

    let mut out_planar: Vec<Vec<f32>> = vec![Vec::new(); channels];
    let mut offset = 0;
    while offset < frame_count {
        let remaining = frame_count - offset;
        let this_chunk = remaining.min(chunk_size);
        let input: Vec<&[f32]> = planar.iter().map(|ch| &ch[offset..offset + this_chunk]).collect();

        let chunk_out = if this_chunk == chunk_size {
            resampler.process(&input, None)?
        } else {
            resampler.process_partial(Some(&input), None)?
        };

        for (ch, data) in chunk_out.into_iter().enumerate() {
            out_planar[ch].extend(data);
        }
        offset += this_chunk;
    }

    // El delay de grupo del filtro sinc agrega/adelanta unas pocas muestras;
    // se recorta al largo esperado para no arrastrar silencio extra.
    let expected_frames = ((frame_count as f64) * ratio).round() as usize;
    for ch in out_planar.iter_mut() {
        ch.truncate(expected_frames);
        while ch.len() < expected_frames {
            ch.push(0.0);
        }
    }

    let mut interleaved = Vec::with_capacity(expected_frames * channels);
    for i in 0..expected_frames {
        for ch in out_planar.iter() {
            interleaved.push(ch[i]);
        }
    }
    Ok(interleaved)
}

/// Escribe WAV f32 estéreo a 48kHz, el formato canónico (PLAN.md §6.1.1 y §6.3 paso 8).
pub fn write_wav(path: &std::path::Path, stereo_interleaved: &[f32]) -> Result<(), NormalizeError> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in stereo_interleaved {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

pub struct ImportResult {
    pub stereo_48k: Vec<f32>,
    pub source_sample_rate: u32,
    pub source_channels: u8,
    pub measured_lufs: f32,
    pub applied_gain_db: f32,
    pub gain_capped: bool,
}

/// Pipeline completo de importación de un pad: decodificar, resamplear,
/// normalizar a estéreo, recortar (si se pide), aplicar micro-fades, medir,
/// calcular y aplicar ganancia (PLAN.md §6.3, SPEC-recorte-v2.md).
///
/// Orden obligatorio: el recorte y los micro-fades ocurren ANTES de medir
/// LUFS — medir sobre el archivo completo cuando el pad es un fragmento
/// rompe la homologación de volúmenes (SPEC-recorte-v2.md, "Orden
/// obligatorio: recortar ANTES de medir").
pub fn import_pipeline(
    path: &std::path::Path,
    target_lufs: f32,
    trim: Option<super::trim::TrimRegion>,
) -> Result<ImportResult, NormalizeError> {
    let decoded = decode_file(path)?;
    let resampled = resample_to_48k(&decoded.samples, decoded.channels, decoded.sample_rate)?;
    let stereo = to_stereo(&resampled, decoded.channels);
    normalize_stereo(stereo, decoded.sample_rate, decoded.channels, target_lufs, trim)
}

/// Segunda mitad de `import_pipeline`, a partir de un buffer ya decodificado,
/// resampleado a 48kHz y convertido a estéreo. Existe por separado para que
/// `commands::analyze_source` pueda cachear ese buffer (decodificar un MP3 de
/// un minuto para dibujar la forma de onda, snapear y luego importar no debe
/// repetirse tres veces — SPEC-recorte-v2.md).
pub fn normalize_stereo(
    mut stereo: Vec<f32>,
    source_sample_rate: u32,
    source_channels: u8,
    target_lufs: f32,
    trim: Option<super::trim::TrimRegion>,
) -> Result<ImportResult, NormalizeError> {
    if let Some(region) = trim {
        let ms_to_frame = |ms: u32| (ms as u64 * TARGET_SAMPLE_RATE as u64 / 1000) as usize;
        let start = super::trim::snap_to_zero_crossing(
            &stereo,
            ms_to_frame(region.start_ms),
            super::trim::ZERO_CROSSING_SEARCH_FRAMES,
        );
        let end = super::trim::snap_to_zero_crossing(
            &stereo,
            ms_to_frame(region.end_ms),
            super::trim::ZERO_CROSSING_SEARCH_FRAMES,
        );
        stereo = super::trim::slice_stereo(&stereo, start, end)?;
    }

    // Micro-fades obligatorios en TODO import, con o sin recorte (SPEC-recorte-v2.md
    // "Micro-fades en los bordes") — independientes de fade_in_ms/fade_out_ms del
    // pad, que son un Tween de kira en tiempo de reproducción, no en el archivo.
    super::trim::apply_edge_fades(&mut stereo, super::trim::MICRO_FADE_FRAMES);

    let measurement = measure_loudness(&stereo, TARGET_SAMPLE_RATE)?;
    let gain = compute_gain(measurement.loudness_lufs, target_lufs, measurement.true_peak_dbtp);
    apply_gain(&mut stereo, gain.gain_db);

    Ok(ImportResult {
        stereo_48k: stereo,
        source_sample_rate,
        source_channels,
        measured_lufs: measurement.loudness_lufs,
        applied_gain_db: gain.gain_db,
        gain_capped: gain.capped,
    })
}

pub struct RenormalizeResult {
    pub measured_lufs: f32,
    pub applied_gain_db: f32,
    pub gain_capped: bool,
}

/// Recalcula la ganancia de un WAV ya normalizado (48kHz estéreo f32) hacia
/// un nuevo `target_lufs` y lo reescribe en el mismo lugar. Mide el archivo
/// TAL COMO ESTÁ ahora — no hace falta "deshacer" la ganancia anterior, medir
/// el resultado actual y apuntar desde ahí al nuevo target da el mismo
/// resultado. Botón "Renormalizar todo" (PLAN.md §6.3).
pub fn renormalize_file(path: &std::path::Path, target_lufs: f32) -> Result<RenormalizeResult, NormalizeError> {
    let mut reader = hound::WavReader::open(path)?;
    let mut samples: Vec<f32> = reader.samples::<f32>().collect::<Result<_, _>>()?;
    drop(reader);

    let measurement = measure_loudness(&samples, TARGET_SAMPLE_RATE)?;
    let gain = compute_gain(measurement.loudness_lufs, target_lufs, measurement.true_peak_dbtp);
    apply_gain(&mut samples, gain.gain_db);
    write_wav(path, &samples)?;

    Ok(RenormalizeResult {
        measured_lufs: measurement.loudness_lufs,
        applied_gain_db: gain.gain_db,
        gain_capped: gain.capped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_stereo_48k_wav(path: &std::path::Path, amplitude: f32, duration_secs: f32) {
        let stereo = sine_tone(amplitude, 1000.0, duration_secs, TARGET_SAMPLE_RATE);
        write_wav(path, &stereo).unwrap();
    }

    #[test]
    fn renormalize_file_moves_an_existing_wav_toward_a_new_target_lufs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pad.wav");
        // -16 LUFS ya normalizado; lo llevamos a un target bien distinto (-23).
        write_stereo_48k_wav(&path, 0.1, 2.0);

        let result = renormalize_file(&path, -23.0).unwrap();

        assert!(!result.gain_capped);
        let reread: Vec<f32> = hound::WavReader::open(&path).unwrap().samples::<f32>().map(|s| s.unwrap()).collect();
        let remeasured = measure_loudness(&reread, TARGET_SAMPLE_RATE).unwrap();
        assert!((remeasured.loudness_lufs - (-23.0)).abs() < 0.5, "quedó en {} LUFS", remeasured.loudness_lufs);
    }

    #[test]
    fn renormalize_file_reports_the_loudness_it_measured_before_renormalizing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pad.wav");
        write_stereo_48k_wav(&path, 0.1, 2.0);

        let first = renormalize_file(&path, -16.0).unwrap();
        let second = renormalize_file(&path, -16.0).unwrap();

        // La segunda vuelta mide el archivo ya normalizado por la primera:
        // debería estar cerca del target anterior y casi no aplicar ganancia.
        assert!((second.measured_lufs - (-16.0)).abs() < 0.5, "midió {}", second.measured_lufs);
        assert!(second.applied_gain_db.abs() < 0.5, "aplicó {}dB", second.applied_gain_db);
        let _ = first;
    }

    fn write_mono_wav(path: &std::path::Path, sample_rate: u32, duration_secs: f32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        let n = (duration_secs * sample_rate as f32) as usize;
        for i in 0..n {
            let t = i as f32 / sample_rate as f32;
            let s = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn decode_and_resample_a_mono_44_1k_wav_to_48k() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        write_mono_wav(&path, 44_100, 1.0);

        let decoded = decode_file(&path).unwrap();
        assert_eq!(decoded.sample_rate, 44_100);
        assert_eq!(decoded.channels, 1);

        let source_frames = decoded.samples.len();
        let resampled = resample_to_48k(&decoded.samples, decoded.channels, decoded.sample_rate).unwrap();
        let expected_frames = (source_frames as f64 * 48_000.0 / 44_100.0).round() as usize;

        assert!(
            (resampled.len() as i64 - expected_frames as i64).abs() <= 4,
            "resampled.len()={} esperado~{}",
            resampled.len(),
            expected_frames
        );
    }

    #[test]
    fn import_pipeline_produces_a_valid_48k_stereo_float_wav() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.wav");
        write_mono_wav(&src_path, 22_050, 1.5);

        let result = import_pipeline(&src_path, -16.0, None).unwrap();
        assert_eq!(result.source_sample_rate, 22_050);
        assert_eq!(result.source_channels, 1);
        assert!(result.measured_lufs.is_finite());
        assert!(result.applied_gain_db.is_finite());

        let out_path = dir.path().join("out.wav");
        write_wav(&out_path, &result.stereo_48k).unwrap();

        let mut reader = hound::WavReader::open(&out_path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 2);
        assert_eq!(spec.sample_rate, 48_000);
        assert_eq!(spec.bits_per_sample, 32);
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);

        let written: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(written.len(), result.stereo_48k.len());
    }

    #[test]
    fn import_pipeline_measures_lufs_of_the_trimmed_fragment_not_the_whole_file() {
        // 55s de un tono fuerte (una música/ambiente de fondo) + 5s de un
        // tono bastante más bajo (el diálogo/efecto que el usuario quiere
        // extraer). El loudness del minuto completo está dominado por los
        // 55s fuertes — mide eso y el fragmento quedaría subido apenas unos
        // dB en vez del boost real que necesita. Recortar ANTES de medir es
        // la única forma de que el pad quede en el target del fragmento, no
        // del archivo completo (SPEC-recorte-v2.md, orden obligatorio).
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.wav");

        let mut full = sine_tone(0.5, 1000.0, 55.0, TARGET_SAMPLE_RATE);
        full.extend(sine_tone(0.02, 1000.0, 5.0, TARGET_SAMPLE_RATE));
        write_wav(&src_path, &full).unwrap();

        let trim = crate::audio::trim::TrimRegion { start_ms: 55_000, end_ms: 60_000 };
        let trimmed = import_pipeline(&src_path, -16.0, Some(trim)).unwrap();
        let untrimmed = import_pipeline(&src_path, -16.0, None).unwrap();

        assert!(
            trimmed.measured_lufs < untrimmed.measured_lufs - 10.0,
            "trimmed={} untrimmed={} — el recorte no está afectando la medición",
            trimmed.measured_lufs,
            untrimmed.measured_lufs
        );

        let remeasured = measure_loudness(&trimmed.stereo_48k, TARGET_SAMPLE_RATE).unwrap();
        assert!(
            (remeasured.loudness_lufs - (-16.0)).abs() < 1.0,
            "el fragmento normalizado quedó en {} LUFS, esperaba ~-16",
            remeasured.loudness_lufs
        );
    }

    #[test]
    fn normalize_stereo_forwards_source_metadata_and_applies_trim() {
        // `normalize_stereo` es el paso "decodificar ya hecho, normalizar
        // desde aquí" que reusa `commands::analyze_source` para no volver a
        // decodificar el archivo (SPEC-recorte-v2.md, cache del editor).
        let stereo = sine_tone(0.2, 1000.0, 5.0, TARGET_SAMPLE_RATE);
        let trim = crate::audio::trim::TrimRegion { start_ms: 1000, end_ms: 3000 };

        let result = normalize_stereo(stereo, 44_100, 1, -16.0, Some(trim)).unwrap();

        assert_eq!(result.source_sample_rate, 44_100);
        assert_eq!(result.source_channels, 1);
        // 2s recortados a 48kHz estéreo (más/menos el ajuste del snap a cruce por cero).
        let frames = result.stereo_48k.len() / 2;
        let expected_frames = 2 * TARGET_SAMPLE_RATE as usize;
        assert!(
            (frames as i64 - expected_frames as i64).abs() < 500,
            "frames={frames} esperado~{expected_frames}"
        );
    }

    #[test]
    fn import_pipeline_applies_micro_fades_to_every_import_even_without_trim() {
        // SPEC-recorte-v2.md: los micro-fades son obligatorios e independientes
        // de fade_in_ms/fade_out_ms del pad, así que se aplican también cuando
        // no hay recorte (decisión de diseño para no dejar un hueco de chasquido
        // en archivos que empiezan/terminan a mitad de un transitorio).
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.wav");
        write_mono_wav(&src_path, 48_000, 1.0);

        let result = import_pipeline(&src_path, -16.0, None).unwrap();

        assert_eq!(result.stereo_48k[0], 0.0, "primera muestra debería quedar en 0 por el fade-in");
    }

    #[test]
    fn import_pipeline_never_exceeds_true_peak_ceiling_even_from_a_loud_source() {
        // Fuente ya casi a 0dBFS: la ganancia de normalización debe recortarse
        // para no pasar de -1dBTP (criterio de aceptación, PLAN.md §14).
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("loud.wav");
        write_mono_wav(&src_path, 48_000, 2.0); // amplitud 0.5 fija en write_mono_wav

        let result = import_pipeline(&src_path, 0.0, None).unwrap(); // target absurdamente alto a propósito
        assert!(result.gain_capped);

        let remeasured = measure_loudness(&result.stereo_48k, TARGET_SAMPLE_RATE).unwrap();
        assert!(
            remeasured.true_peak_dbtp <= TRUE_PEAK_CEILING_DBTP + 0.2,
            "true_peak tras normalizar: {}",
            remeasured.true_peak_dbtp
        );
    }

    /// Tono senoidal estéreo intercalado, ambos canales iguales.
    fn sine_tone(amplitude: f32, freq_hz: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
        let n = (duration_secs * sample_rate as f32) as usize;
        (0..n)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let s = (t * freq_hz * std::f32::consts::TAU).sin() * amplitude;
                [s, s]
            })
            .collect()
    }

    #[test]
    fn measure_loudness_of_a_4s_tone_uses_integrated_branch_and_scales_6db_with_amplitude() {
        // >= 3s: debe usar loudness_global(). Duplicar la amplitud debe leerse ~6.02 dB más alto.
        let quiet = sine_tone(0.1, 1000.0, 4.0, TARGET_SAMPLE_RATE);
        let loud = sine_tone(0.2, 1000.0, 4.0, TARGET_SAMPLE_RATE);

        let m_quiet = measure_loudness(&quiet, TARGET_SAMPLE_RATE).unwrap();
        let m_loud = measure_loudness(&loud, TARGET_SAMPLE_RATE).unwrap();

        let diff = m_loud.loudness_lufs - m_quiet.loudness_lufs;
        assert!((diff - 6.0206).abs() < 0.2, "diff fue {diff}, esperaba ~6.02 dB");
    }

    #[test]
    fn measure_loudness_of_a_1s_tone_uses_ungated_window_branch_and_scales_6db_with_amplitude() {
        // < 3s: debe usar loudness_window() sin gating (PLAN.md §6.3).
        let quiet = sine_tone(0.1, 1000.0, 1.0, TARGET_SAMPLE_RATE);
        let loud = sine_tone(0.2, 1000.0, 1.0, TARGET_SAMPLE_RATE);

        let m_quiet = measure_loudness(&quiet, TARGET_SAMPLE_RATE).unwrap();
        let m_loud = measure_loudness(&loud, TARGET_SAMPLE_RATE).unwrap();

        let diff = m_loud.loudness_lufs - m_quiet.loudness_lufs;
        assert!((diff - 6.0206).abs() < 0.2, "diff fue {diff}, esperaba ~6.02 dB");
    }

    #[test]
    fn measure_loudness_reports_true_peak_close_to_expected_dbfs() {
        // Amplitud 0.5 ~ -6.02 dBFS de pico. True peak puede sobrepasar levemente
        // el sample peak por interpolación entre muestras; se deja margen amplio.
        let tone = sine_tone(0.5, 1000.0, 1.0, TARGET_SAMPLE_RATE);

        let m = measure_loudness(&tone, TARGET_SAMPLE_RATE).unwrap();

        assert!(m.true_peak_dbtp > -8.0 && m.true_peak_dbtp < -4.0, "true_peak fue {}", m.true_peak_dbtp);
    }

    #[test]
    fn to_stereo_duplicates_mono_to_both_channels() {
        let mono = vec![0.1, 0.2, -0.5];

        let stereo = to_stereo(&mono, 1);

        assert_eq!(stereo, vec![0.1, 0.1, 0.2, 0.2, -0.5, -0.5]);
    }

    #[test]
    fn to_stereo_leaves_stereo_unchanged() {
        let stereo_in = vec![0.1, -0.1, 0.2, -0.2];

        let stereo_out = to_stereo(&stereo_in, 2);

        assert_eq!(stereo_out, stereo_in);
    }

    #[test]
    fn to_stereo_keeps_only_first_two_channels_of_multichannel() {
        // 3 canales (L, R, C) intercalados: un frame = [L, R, C]
        let multi = vec![0.1, 0.2, 0.9, 0.3, 0.4, 0.9];

        let stereo = to_stereo(&multi, 3);

        assert_eq!(stereo, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn apply_gain_of_zero_db_leaves_samples_unchanged() {
        let mut samples = vec![0.1, -0.2, 0.3];

        apply_gain(&mut samples, 0.0);

        assert_eq!(samples, vec![0.1, -0.2, 0.3]);
    }

    #[test]
    fn apply_gain_of_plus_6db_roughly_doubles_amplitude() {
        let mut samples = vec![0.1, 0.2];

        apply_gain(&mut samples, 6.0206);

        assert!((samples[0] - 0.2).abs() < 0.001);
        assert!((samples[1] - 0.4).abs() < 0.001);
    }

    #[test]
    fn compute_gain_reaches_target_when_there_is_headroom() {
        // measured -20 LUFS, target -16 LUFS -> +4dB. Techo en -10dBTP da 9dB de margen.
        let result = compute_gain(-20.0, -16.0, -10.0);

        assert!((result.gain_db - 4.0).abs() < 0.001);
        assert!(!result.capped);
    }

    #[test]
    fn compute_gain_caps_at_true_peak_ceiling() {
        // measured -30 LUFS, target -16 LUFS pediría +14dB, pero el pico ya está en -5dBTP.
        // Techo -1dBTP => solo hay 4dB de margen.
        let result = compute_gain(-30.0, -16.0, -5.0);

        assert!((result.gain_db - 4.0).abs() < 0.001);
        assert!(result.capped);
    }
}
