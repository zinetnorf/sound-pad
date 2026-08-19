use crate::models::RetriggerMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerAction {
    /// Pad inactivo: arrancar normalmente.
    Start,
    /// Modo Stop, ya sonando: detenerlo y no arrancar uno nuevo.
    StopOnly,
    /// Modo Restart, ya sonando: detener y arrancar de nuevo desde el principio.
    RestartFromBeginning,
    /// Modo Ignore, ya sonando: no hacer nada.
    DoNothing,
    /// Modo Overlap: sumar una voz. Si ya está en el tope, expulsar la más vieja primero.
    AddOverlap { evict_oldest: bool },
}

/// Decide qué hacer al disparar un pad según su modo de re-disparo y cuántas
/// voces tiene activas ahora mismo. Pura — no toca el motor. PLAN.md §7 y §6.5.
pub fn decide_trigger_action(mode: RetriggerMode, active_count: usize, max_overlap: usize) -> TriggerAction {
    if active_count == 0 {
        return TriggerAction::Start;
    }
    match mode {
        RetriggerMode::Stop => TriggerAction::StopOnly,
        RetriggerMode::Restart => TriggerAction::RestartFromBeginning,
        RetriggerMode::Ignore => TriggerAction::DoNothing,
        RetriggerMode::Overlap => TriggerAction::AddOverlap { evict_oldest: active_count >= max_overlap },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RetriggerMode;

    #[test]
    fn always_starts_when_pad_is_idle_regardless_of_mode() {
        for mode in [RetriggerMode::Stop, RetriggerMode::Restart, RetriggerMode::Ignore, RetriggerMode::Overlap] {
            assert_eq!(decide_trigger_action(mode, 0, 8), TriggerAction::Start, "mode={mode:?}");
        }
    }

    #[test]
    fn stop_mode_stops_without_restarting_when_already_playing() {
        assert_eq!(decide_trigger_action(RetriggerMode::Stop, 1, 8), TriggerAction::StopOnly);
    }

    #[test]
    fn restart_mode_stops_and_plays_again_from_the_beginning() {
        assert_eq!(decide_trigger_action(RetriggerMode::Restart, 1, 8), TriggerAction::RestartFromBeginning);
    }

    #[test]
    fn ignore_mode_does_nothing_when_already_playing() {
        assert_eq!(decide_trigger_action(RetriggerMode::Ignore, 1, 8), TriggerAction::DoNothing);
    }

    #[test]
    fn overlap_mode_adds_a_voice_without_evicting_when_under_the_cap() {
        assert_eq!(
            decide_trigger_action(RetriggerMode::Overlap, 3, 8),
            TriggerAction::AddOverlap { evict_oldest: false }
        );
    }

    #[test]
    fn overlap_mode_evicts_the_oldest_voice_when_at_the_cap() {
        assert_eq!(
            decide_trigger_action(RetriggerMode::Overlap, 8, 8),
            TriggerAction::AddOverlap { evict_oldest: true }
        );
    }
}
