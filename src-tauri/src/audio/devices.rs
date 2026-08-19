#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub label: String,
}

/// Enumera los dispositivos de salida reales vía cpal. Impura — no testeable
/// sin hardware; la lógica de decisión vive en `resolve_device`, que sí lo es.
pub fn enumerate() -> Vec<DeviceInfo> {
    use cpal::traits::HostTrait;
    let host = cpal::default_host();
    host.output_devices()
        .map(|devices| devices.filter_map(device_info).collect())
        .unwrap_or_default()
}

pub fn default_device_id() -> Option<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.id().ok())
        .map(|id| id.to_string())
}

fn device_info(d: cpal::Device) -> Option<DeviceInfo> {
    use cpal::traits::DeviceTrait;
    let id = d.id().ok()?.to_string();
    let label = d.description().ok()?.to_string();
    Some(DeviceInfo { id, label })
}

/// Busca el `cpal::Device` real cuyo id coincide. Impura.
pub fn find_by_id(id: &str) -> Option<cpal::Device> {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host()
        .output_devices()
        .ok()?
        .find(|d| d.id().map(|i| i.to_string() == id).unwrap_or(false))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    ById,
    ByLabel,
    FellBackToDefault,
}

/// Resuelve qué dispositivo abrir: por id, luego por label, luego el default.
/// Pura — no toca cpal, testeable sin hardware.
pub fn resolve_device<'a>(
    available: &'a [DeviceInfo],
    wanted_id: &str,
    wanted_label: &str,
    default_id: &str,
) -> (&'a DeviceInfo, ResolutionKind) {
    if let Some(d) = available.iter().find(|d| d.id == wanted_id) {
        return (d, ResolutionKind::ById);
    }
    if let Some(d) = available.iter().find(|d| d.label == wanted_label) {
        return (d, ResolutionKind::ByLabel);
    }
    let fallback = available
        .iter()
        .find(|d| d.id == default_id)
        .or_else(|| available.first())
        .expect("no hay dispositivos de salida disponibles");
    (fallback, ResolutionKind::FellBackToDefault)
}

/// Un rango de config soportado por el dispositivo (espejo simplificado
/// de `cpal::SupportedStreamConfigRange`, sin depender de cpal).
#[derive(Debug, Clone, Copy)]
pub struct SupportedRange {
    pub min_sample_rate: u32,
    pub max_sample_rate: u32,
    pub min_buffer_frames: u32,
    pub max_buffer_frames: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigChoice {
    pub sample_rate: u32,
    pub buffer_frames: u32,
    pub matches_target: bool,
}

/// Elige la config más cercana a (target_sample_rate, target_buffer_frames)
/// entre los rangos soportados. Pura — no toca cpal.
pub fn negotiate_config(
    ranges: &[SupportedRange],
    target_sample_rate: u32,
    target_buffer_frames: u32,
) -> ConfigChoice {
    let range = ranges
        .iter()
        .find(|r| target_sample_rate >= r.min_sample_rate && target_sample_rate <= r.max_sample_rate)
        .or_else(|| ranges.first())
        .expect("no hay configs soportadas");

    let sample_rate = target_sample_rate.clamp(range.min_sample_rate, range.max_sample_rate);
    let buffer_frames = target_buffer_frames.clamp(range.min_buffer_frames, range.max_buffer_frames);

    ConfigChoice {
        sample_rate,
        buffer_frames,
        matches_target: sample_rate == target_sample_rate && buffer_frames == target_buffer_frames,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_by_device_id_when_present() {
        let available = vec![
            DeviceInfo { id: "usb-mixer".into(), label: "USB Mixer".into() },
            DeviceInfo { id: "builtin".into(), label: "MacBook Speakers".into() },
        ];

        let (device, kind) = resolve_device(&available, "usb-mixer", "USB Mixer", "builtin");

        assert_eq!(device.id, "usb-mixer");
        assert_eq!(kind, ResolutionKind::ById);
    }

    #[test]
    fn falls_back_to_label_when_device_id_changed() {
        // El OS reasignó el id (p.ej. tras reconectar el USB) pero el label sigue igual.
        let available = vec![
            DeviceInfo { id: "usb-mixer-v2".into(), label: "USB Mixer".into() },
            DeviceInfo { id: "builtin".into(), label: "MacBook Speakers".into() },
        ];

        let (device, kind) = resolve_device(&available, "usb-mixer-v1", "USB Mixer", "builtin");

        assert_eq!(device.id, "usb-mixer-v2");
        assert_eq!(kind, ResolutionKind::ByLabel);
    }

    #[test]
    fn falls_back_to_default_when_neither_id_nor_label_exist() {
        let available = vec![
            DeviceInfo { id: "builtin".into(), label: "MacBook Speakers".into() },
            DeviceInfo { id: "airpods".into(), label: "AirPods".into() },
        ];

        let (device, kind) = resolve_device(&available, "usb-mixer", "USB Mixer", "builtin");

        assert_eq!(device.id, "builtin");
        assert_eq!(kind, ResolutionKind::FellBackToDefault);
    }

    #[test]
    fn negotiate_config_matches_target_when_range_supports_it() {
        let ranges = vec![SupportedRange {
            min_sample_rate: 44100,
            max_sample_rate: 96000,
            min_buffer_frames: 32,
            max_buffer_frames: 2048,
        }];

        let choice = negotiate_config(&ranges, 48000, 256);

        assert_eq!(choice, ConfigChoice { sample_rate: 48000, buffer_frames: 256, matches_target: true });
    }

    #[test]
    fn negotiate_config_clamps_buffer_when_device_forces_larger_minimum() {
        // Dongle barato: no acepta buffers menores a 512.
        let ranges = vec![SupportedRange {
            min_sample_rate: 44100,
            max_sample_rate: 48000,
            min_buffer_frames: 512,
            max_buffer_frames: 4096,
        }];

        let choice = negotiate_config(&ranges, 48000, 256);

        assert_eq!(choice, ConfigChoice { sample_rate: 48000, buffer_frames: 512, matches_target: false });
    }

    #[test]
    fn negotiate_config_clamps_sample_rate_when_48k_unsupported() {
        let ranges = vec![SupportedRange {
            min_sample_rate: 8000,
            max_sample_rate: 44100,
            min_buffer_frames: 32,
            max_buffer_frames: 2048,
        }];

        let choice = negotiate_config(&ranges, 48000, 256);

        assert_eq!(choice, ConfigChoice { sample_rate: 44100, buffer_frames: 256, matches_target: false });
    }
}
