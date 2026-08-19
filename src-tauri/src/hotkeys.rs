use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::models::AppConfig;

/// Traduce una tecla asignable ("A".."Z", "0".."9", PLAN.md §8) al `Code`
/// del teclado. Pura.
pub fn code_for_key(key: &str) -> Option<Code> {
    let upper = key.to_uppercase();
    let mut chars = upper.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    match c {
        'A' => Some(Code::KeyA),
        'B' => Some(Code::KeyB),
        'C' => Some(Code::KeyC),
        'D' => Some(Code::KeyD),
        'E' => Some(Code::KeyE),
        'F' => Some(Code::KeyF),
        'G' => Some(Code::KeyG),
        'H' => Some(Code::KeyH),
        'I' => Some(Code::KeyI),
        'J' => Some(Code::KeyJ),
        'K' => Some(Code::KeyK),
        'L' => Some(Code::KeyL),
        'M' => Some(Code::KeyM),
        'N' => Some(Code::KeyN),
        'O' => Some(Code::KeyO),
        'P' => Some(Code::KeyP),
        'Q' => Some(Code::KeyQ),
        'R' => Some(Code::KeyR),
        'S' => Some(Code::KeyS),
        'T' => Some(Code::KeyT),
        'U' => Some(Code::KeyU),
        'V' => Some(Code::KeyV),
        'W' => Some(Code::KeyW),
        'X' => Some(Code::KeyX),
        'Y' => Some(Code::KeyY),
        'Z' => Some(Code::KeyZ),
        '0' => Some(Code::Digit0),
        '1' => Some(Code::Digit1),
        '2' => Some(Code::Digit2),
        '3' => Some(Code::Digit3),
        '4' => Some(Code::Digit4),
        '5' => Some(Code::Digit5),
        '6' => Some(Code::Digit6),
        '7' => Some(Code::Digit7),
        '8' => Some(Code::Digit8),
        '9' => Some(Code::Digit9),
        _ => None,
    }
}

/// Parsea el modificador configurable (default "Ctrl+Alt", PLAN.md §8) a
/// `Modifiers`. Tokens desconocidos se ignoran en vez de fallar — mejor un
/// atajo parcial que tirar la app abajo por una config vieja o mal escrita.
pub fn modifiers_for(spec: &str) -> Modifiers {
    let mut mods = Modifiers::empty();
    for part in spec.split('+') {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "option" | "opt" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            "cmd" | "super" | "meta" | "command" => mods |= Modifiers::SUPER,
            _ => {}
        }
    }
    mods
}

/// Re-registra los atajos globales (⌃⌥+tecla, PLAN.md §8) para el banco
/// activo. Se llama al arrancar y cada vez que cambia la config — pads,
/// hotkeys, el modificador o el on/off global. Reemplaza todo lo anterior
/// en vez de hacer un diff, porque re-registrar 40 atajos es instantáneo y
/// mucho más simple que llevar la cuenta de qué cambió.
pub fn refresh(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();

    if !config.global_hotkeys_enabled {
        return Ok(());
    }
    let mods = modifiers_for(&config.global_modifier);

    if let Some(key) = &config.panic_hotkey {
        if let Some(code) = code_for_key(key) {
            let handler_app = app.clone();
            let result = manager.on_shortcut(Shortcut::new(Some(mods), code), move |_app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    if let Err(e) = crate::commands::panic_now(&handler_app) {
                        eprintln!("atajo global de pánico falló: {e}");
                    }
                }
            });
            if let Err(e) = result {
                eprintln!("no se pudo registrar el atajo de pánico: {e}");
            }
        }
    }

    let Some(bank) = config.banks.iter().find(|b| b.id == config.active_bank_id) else {
        return Ok(());
    };

    for pad in &bank.pads {
        let Some(key) = &pad.hotkey else { continue };
        let Some(code) = code_for_key(key) else { continue };

        let shortcut = Shortcut::new(Some(mods), code);
        let bank_id = bank.id.clone();
        let pad_id = pad.id.clone();
        let pad_name = pad.name.clone();
        let handler_app = app.clone();

        let result = manager.on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Err(e) = crate::commands::trigger(&handler_app, &bank_id, &pad_id) {
                    eprintln!("atajo global de \"{pad_name}\" falló: {e}");
                }
            }
        });
        if let Err(e) = result {
            eprintln!("no se pudo registrar el atajo de \"{}\": {e}", pad.name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Code;

    #[test]
    fn code_for_key_maps_letters() {
        assert_eq!(code_for_key("A"), Some(Code::KeyA));
        assert_eq!(code_for_key("a"), Some(Code::KeyA));
        assert_eq!(code_for_key("Z"), Some(Code::KeyZ));
    }

    #[test]
    fn code_for_key_maps_digits() {
        assert_eq!(code_for_key("0"), Some(Code::Digit0));
        assert_eq!(code_for_key("9"), Some(Code::Digit9));
    }

    #[test]
    fn code_for_key_rejects_anything_else() {
        assert_eq!(code_for_key(""), None);
        assert_eq!(code_for_key("AB"), None);
        assert_eq!(code_for_key("!"), None);
        assert_eq!(code_for_key("ñ"), None);
    }

    #[test]
    fn modifiers_for_parses_ctrl_alt() {
        assert_eq!(modifiers_for("Ctrl+Alt"), Modifiers::CONTROL | Modifiers::ALT);
    }

    #[test]
    fn modifiers_for_is_case_and_space_insensitive() {
        assert_eq!(modifiers_for("ctrl + alt"), Modifiers::CONTROL | Modifiers::ALT);
    }

    #[test]
    fn modifiers_for_ignores_unknown_tokens() {
        assert_eq!(modifiers_for("Ctrl+Bogus"), Modifiers::CONTROL);
    }
}
