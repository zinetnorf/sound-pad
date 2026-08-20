//! Recorte de audio al importar (SPEC-recorte-v2.md). Funciones puras —
//! el orden obligatorio (recortar -> micro-fades -> medir LUFS) se aplica
//! en `normalize::import_pipeline`, no aquí.

/// Región a conservar de un import, en milisegundos sobre el audio ya
/// resampleado a 48kHz. `end_ms` es exclusivo. Nunca se persiste — el
/// recorte es destructivo (SPEC-recorte-v2.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrimRegion {
    pub start_ms: u32,
    pub end_ms: u32,
}

/// Ancho de la búsqueda de cruce por cero alrededor de cada handle: 5ms a
/// 48kHz. Suficiente para encontrar un cruce en audio real sin desplazar el
/// corte de forma perceptible (SPEC-recorte-v2.md: "imperceptible visualmente").
pub const ZERO_CROSSING_SEARCH_FRAMES: usize = 240;

/// Duración del micro-fade obligatorio en los bordes: 5ms a 48kHz.
pub const MICRO_FADE_FRAMES: usize = 240;

/// Mueve `frame` al cruce por cero más cercano (en el canal izquierdo),
/// buscando hasta `max_search_frames` en ambas direcciones. Un "cruce" es
/// una muestra exactamente en 0.0, o el punto entre dos muestras de signo
/// opuesto — en audio real casi nunca hay un 0.0 exacto. Si no hay cruce
/// en la ventana, devuelve `frame` sin cambios. Idempotente: llamarla dos
/// veces seguidas da el mismo resultado (SPEC-recorte-v2.md "Snap a cruce
/// por cero").
pub fn snap_to_zero_crossing(stereo: &[f32], frame: usize, max_search_frames: usize) -> usize {
    let frame_count = stereo.len() / 2;
    if frame_count == 0 {
        return frame;
    }
    let frame = frame.min(frame_count - 1);
    let left = |i: usize| stereo[i * 2];

    if left(frame) == 0.0 {
        return frame;
    }

    // Candidato de una dirección: la muestra exacta en cero más cercana, o
    // el índice justo antes de un cambio de signo (el más cercano a `frame`
    // de los dos lados del cruce).
    let scan = |range: Box<dyn Iterator<Item = usize>>| -> Option<usize> {
        let mut prev = frame;
        for i in range {
            if left(i) == 0.0 {
                return Some(i);
            }
            if left(i).signum() != left(prev).signum() {
                // El cruce cae entre `prev` e `i`; nos quedamos con el que
                // esté más cerca del frame original.
                return Some(if i.abs_diff(frame) <= prev.abs_diff(frame) { i } else { prev });
            }
            prev = i;
        }
        None
    };

    let backward = scan(Box::new((frame.saturating_sub(max_search_frames)..frame).rev()));
    let forward = scan(Box::new((frame + 1)..=(frame + max_search_frames).min(frame_count - 1)));

    match (backward, forward) {
        (Some(b), Some(f)) => {
            if frame.abs_diff(b) <= frame.abs_diff(f) {
                b
            } else {
                f
            }
        }
        (Some(b), None) => b,
        (None, Some(f)) => f,
        (None, None) => frame,
    }
}

/// Extrae el rango de frames `[start_frame, end_frame)` de un buffer
/// estéreo intercalado. `end_frame` es exclusivo.
pub fn slice_stereo(stereo: &[f32], start_frame: usize, end_frame: usize) -> Result<Vec<f32>, TrimError> {
    let frame_count = stereo.len() / 2;
    if end_frame <= start_frame {
        return Err(TrimError::InvalidRange { start_frame, end_frame });
    }
    if end_frame > frame_count {
        return Err(TrimError::OutOfBounds { end_frame, frame_count });
    }
    Ok(stereo[start_frame * 2..end_frame * 2].to_vec())
}

/// Aplica una rampa lineal de `fade_frames` en ambos bordes del buffer,
/// escrita EN las muestras — independiente de fade_in_ms/fade_out_ms del
/// pad, que son un Tween de kira en tiempo de reproducción (SPEC-recorte-v2.md
/// "Micro-fades en los bordes"). Si el clip es más corto que `2 * fade_frames`,
/// el fade-in y el fade-out se acortan a la mitad del clip para no solaparse
/// de forma inconsistente.
pub fn apply_edge_fades(stereo: &mut [f32], fade_frames: usize) {
    let frame_count = stereo.len() / 2;
    if fade_frames == 0 || frame_count == 0 {
        return;
    }
    let fade_frames = fade_frames.min(frame_count / 2).max(if frame_count == 1 { 0 } else { 1 });
    if fade_frames == 0 {
        return;
    }

    for i in 0..fade_frames {
        let gain = i as f32 / fade_frames as f32;
        stereo[i * 2] *= gain;
        stereo[i * 2 + 1] *= gain;

        let end_frame = frame_count - 1 - i;
        stereo[end_frame * 2] *= gain;
        stereo[end_frame * 2 + 1] *= gain;
    }
}

/// Pico con signo por bucket, un `Vec` por canal (SPEC-recorte-v2.md "Forma
/// de onda"): para no mandar ~5.7M muestras crudas al WebView, se reduce a
/// `buckets` puntos por canal, cada uno el valor de mayor magnitud (con su
/// signo original) dentro de ese tramo. wavesurfer.js v7 dibuja un array
/// plano por canal, no pares min/max intercalados — enviar solo el valor
/// absoluto aplanaría la mitad inferior de la onda.
pub fn compute_peaks(stereo: &[f32], buckets: usize) -> Vec<Vec<f32>> {
    let frame_count = stereo.len() / 2;
    let mut left = vec![0.0_f32; buckets];
    let mut right = vec![0.0_f32; buckets];

    if buckets == 0 || frame_count == 0 {
        return vec![left, right];
    }

    for b in 0..buckets {
        let start = b * frame_count / buckets;
        let end = ((b + 1) * frame_count / buckets).max(start + 1).min(frame_count);

        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        for frame in start..end {
            let l = stereo[frame * 2];
            let r = stereo[frame * 2 + 1];
            if l.abs() > peak_l.abs() {
                peak_l = l;
            }
            if r.abs() > peak_r.abs() {
                peak_r = r;
            }
        }
        left[b] = peak_l;
        right[b] = peak_r;
    }

    vec![left, right]
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TrimError {
    #[error("rango de recorte inválido: start_frame={start_frame} >= end_frame={end_frame}")]
    InvalidRange { start_frame: usize, end_frame: usize },
    #[error("end_frame={end_frame} excede el buffer de {frame_count} frames")]
    OutOfBounds { end_frame: usize, frame_count: usize },
}

/// Convierte un rango en milisegundos a un rango de frames, recortado
/// contra el largo real del buffer. Nunca devuelve un `end` (ni un `start`)
/// mayor a `frame_count`, sin importar qué ms lleguen del frontend —
/// obsoletos por un drag rápido, o simplemente el usuario soltando el
/// handle justo en el final del clip. Usado por el preview del editor de
/// recorte: antes se armaba la región directo con la conversión ms→muestras
/// en punto flotante de kira, y un redondeo hacia arriba en el borde exacto
/// del buffer producía un índice un frame más allá del final — pánico que
/// aborta el proceso entero, porque un panic de Rust no puede cruzar el
/// límite del motor de audio en tiempo real (SPEC-recorte-v2.md).
pub fn clamp_ms_range_to_frames(start_ms: u32, end_ms: u32, frame_count: usize, sample_rate: u32) -> (usize, usize) {
    let ms_to_frame = |ms: u32| ((ms as u64 * sample_rate as u64) / 1000) as usize;
    let start = ms_to_frame(start_ms).min(frame_count);
    let end = ms_to_frame(end_ms).min(frame_count).max(start);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_to_zero_crossing_does_not_move_a_frame_already_at_zero() {
        // Estéreo intercalado: frame 1 (índice de muestra 2..4) ya está en cero.
        let stereo = vec![0.5, 0.5, 0.0, 0.0, -0.5, -0.5];

        let snapped = snap_to_zero_crossing(&stereo, 1, 10);

        assert_eq!(snapped, 1);
    }

    #[test]
    fn snap_to_zero_crossing_finds_nearest_sign_change_when_no_exact_zero_exists() {
        // Canal izquierdo: +0.5, +0.3, -0.2 (signo cambia entre frame 1 y 2),
        // +0.4, +0.6. Ningún valor es exactamente 0.0.
        let left = [0.5_f32, 0.3, -0.2, 0.4, 0.6];
        let stereo: Vec<f32> = left.iter().flat_map(|&s| [s, s]).collect();

        // Empezar en frame 0 (0.5): el cruce más cercano es entre frame 1 y 2.
        let snapped = snap_to_zero_crossing(&stereo, 0, 10);

        assert!(snapped == 1 || snapped == 2, "snapped fue {snapped}, esperaba 1 o 2");
    }

    #[test]
    fn snap_to_zero_crossing_returns_original_frame_when_no_crossing_in_window() {
        // Todo el buffer es positivo: no hay cruce por cero en ningún lado.
        let left = [0.5_f32, 0.4, 0.3, 0.6, 0.7];
        let stereo: Vec<f32> = left.iter().flat_map(|&s| [s, s]).collect();

        let snapped = snap_to_zero_crossing(&stereo, 2, 1);

        assert_eq!(snapped, 2);
    }

    #[test]
    fn snap_to_zero_crossing_is_idempotent() {
        let left = [0.5_f32, 0.3, -0.2, 0.4, 0.6];
        let stereo: Vec<f32> = left.iter().flat_map(|&s| [s, s]).collect();

        let once = snap_to_zero_crossing(&stereo, 0, 10);
        let twice = snap_to_zero_crossing(&stereo, once, 10);

        assert_eq!(once, twice);
    }

    #[test]
    fn slice_stereo_extracts_the_requested_frame_range() {
        // 4 frames: (0,0) (1,1) (2,2) (3,3)
        let stereo = vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0];

        let sliced = slice_stereo(&stereo, 1, 3).unwrap();

        assert_eq!(sliced, vec![1.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn slice_stereo_rejects_end_before_or_equal_to_start() {
        let stereo = vec![0.0, 0.0, 1.0, 1.0];

        assert!(slice_stereo(&stereo, 1, 1).is_err());
        assert!(slice_stereo(&stereo, 2, 1).is_err());
    }

    #[test]
    fn slice_stereo_rejects_end_past_the_buffer() {
        let stereo = vec![0.0, 0.0, 1.0, 1.0];

        assert!(slice_stereo(&stereo, 0, 3).is_err());
    }

    #[test]
    fn apply_edge_fades_ramps_the_first_and_last_frames_to_near_zero() {
        // 10 frames a amplitud constante 1.0 en ambos canales.
        let mut stereo: Vec<f32> = (0..10).flat_map(|_| [1.0_f32, 1.0]).collect();

        apply_edge_fades(&mut stereo, 4);

        // Primer frame: ganancia 0 (rampa lineal 0/4..4/4 con 4 frames de fade).
        assert!(stereo[0].abs() < 0.01, "primer frame quedó en {}", stereo[0]);
        // Último frame: ganancia 0 también, por simetría del fade-out.
        let last = stereo.len() - 2;
        assert!(stereo[last].abs() < 0.01, "último frame quedó en {}", stereo[last]);
    }

    #[test]
    fn apply_edge_fades_leaves_the_center_untouched() {
        let mut stereo: Vec<f32> = (0..10).flat_map(|_| [1.0_f32, 1.0]).collect();

        apply_edge_fades(&mut stereo, 2);

        // Centro (frames 2..8, fuera de la ventana de fade de 2 frames por lado).
        for frame in 2..8 {
            assert_eq!(stereo[frame * 2], 1.0, "frame {frame} se tocó sin deber");
        }
    }

    #[test]
    fn apply_edge_fades_with_zero_frames_leaves_buffer_unchanged() {
        let original: Vec<f32> = (0..10).flat_map(|_| [1.0_f32, 1.0]).collect();
        let mut stereo = original.clone();

        apply_edge_fades(&mut stereo, 0);

        assert_eq!(stereo, original);
    }

    #[test]
    fn apply_edge_fades_clamps_when_fade_is_longer_than_the_clip() {
        // Clip de 3 frames, fade pedido de 10: no debe entrar en pánico ni
        // hacer overlap raro entre el fade-in y el fade-out.
        let mut stereo: Vec<f32> = (0..3).flat_map(|_| [1.0_f32, 1.0]).collect();

        apply_edge_fades(&mut stereo, 10);

        assert!(stereo.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn compute_peaks_returns_one_vec_per_channel_with_the_requested_bucket_count() {
        let stereo: Vec<f32> = (0..100).flat_map(|_| [0.1_f32, -0.1]).collect();

        let peaks = compute_peaks(&stereo, 10);

        assert_eq!(peaks.len(), 2, "debe haber un Vec por canal");
        assert_eq!(peaks[0].len(), 10);
        assert_eq!(peaks[1].len(), 10);
    }

    #[test]
    fn compute_peaks_keeps_the_signed_sample_of_largest_magnitude_per_bucket() {
        // 4 frames en un solo bucket: el pico de mayor magnitud es -0.9 (canal L).
        let left = [0.1_f32, -0.9, 0.5, -0.3];
        let right = [0.2_f32, 0.2, 0.2, 0.2];
        let stereo: Vec<f32> = left.iter().zip(right.iter()).flat_map(|(&l, &r)| [l, r]).collect();

        let peaks = compute_peaks(&stereo, 1);

        assert_eq!(peaks[0][0], -0.9);
        assert_eq!(peaks[1][0], 0.2);
    }

    #[test]
    fn compute_peaks_of_silence_is_all_zero() {
        let stereo = vec![0.0_f32; 200];

        let peaks = compute_peaks(&stereo, 5);

        assert!(peaks[0].iter().all(|&p| p == 0.0));
        assert!(peaks[1].iter().all(|&p| p == 0.0));
    }

    #[test]
    fn compute_peaks_handles_fewer_frames_than_buckets() {
        // 3 frames pedidos en 10 buckets: no debe entrar en pánico por división por cero.
        let stereo: Vec<f32> = (0..3).flat_map(|_| [0.5_f32, 0.5]).collect();

        let peaks = compute_peaks(&stereo, 10);

        assert_eq!(peaks[0].len(), 10);
    }

    #[test]
    fn clamp_ms_range_to_frames_converts_ms_to_frames_at_48khz() {
        let (start, end) = clamp_ms_range_to_frames(10, 20, 10_000, 48_000);

        assert_eq!(start, 480);
        assert_eq!(end, 960);
    }

    #[test]
    fn clamp_ms_range_to_frames_clamps_end_that_rounds_past_the_buffer() {
        // Reproduce el crash real: end_ms corresponde, en punto flotante, a un
        // frame exactamente 1 más allá del buffer (redondeo de ms/1000*rate
        // hacia arriba). El resultado nunca debe exceder frame_count, o el
        // preview del editor de recorte revienta el proceso entero (los
        // panics de Rust no cruzan el límite del motor de audio: abortan).
        let frame_count = 893_400;
        let (_, end) = clamp_ms_range_to_frames(0, 18_613, frame_count, 48_000);

        assert!(end <= frame_count, "end={end} excede frame_count={frame_count}");
    }

    #[test]
    fn clamp_ms_range_to_frames_clamps_start_past_the_buffer_too() {
        let (start, end) = clamp_ms_range_to_frames(999_999, 999_999, 100, 48_000);

        assert!(start <= 100, "start={start} excede frame_count=100");
        assert!(end <= 100, "end={end} excede frame_count=100");
    }

    #[test]
    fn clamp_ms_range_to_frames_never_returns_end_before_start() {
        // start_ms mayor que end_ms (valores obsoletos de un drag rápido en
        // el frontend) no debe producir un rango invertido.
        let (start, end) = clamp_ms_range_to_frames(500, 100, 10_000, 48_000);

        assert!(end >= start, "start={start} end={end}");
    }

    #[test]
    fn clamp_ms_range_to_frames_of_an_empty_buffer_is_zero() {
        let (start, end) = clamp_ms_range_to_frames(0, 0, 0, 48_000);

        assert_eq!(start, 0);
        assert_eq!(end, 0);
    }
}
