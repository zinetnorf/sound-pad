// Diccionario de la interfaz. Los nombres de pads y bancos son datos del
// usuario y NUNCA pasan por acá — se muestran tal cual los escribió.

export interface Dictionary {
  common: {
    close: string;
    cancel: string;
    save: string;
    yes: string;
    no: string;
    remove: string;
    edit: string;
    done: string;
    name: string;
    nameTooltip: string;
    namePlaceholder: string;
    color: string;
    colorTooltip: string;
    hotkey: string;
    hotkeyTooltip: string;
    hotkeyBackgroundNote: (key: string) => string;
    midiController: string;
    midiControllerTooltip: string;
    errorPrefix: (err: string) => string;
    triggerError: (err: string) => string;
    saveError: (err: string) => string;
    deleteError: (err: string) => string;
  };
  app: {
    addPad: string;
    settingsTitle: string;
    config: string;
    panic: string;
    panicTitle: string;
    live: string;
    editMode: string;
    noBanksHint: string;
    deviceLost: (label: string) => string;
  };
  settings: {
    title: string;
    outputDevice: string;
    outputDeviceTooltip: string;
    testDeviceTitle: string;
    test: string;
    playing: string;
    midiDevice: string;
    midiDeviceTooltip: string;
    noMidiDevice: string;
    languageTooltip: string;
    gridColumns: string;
    gridColumnsTooltip: string;
    targetLufs: string;
    targetLufsTooltip: string;
    renormalizeAll: string;
    renormalizing: string;
    globalHotkeys: string;
    globalHotkeysTooltip: string;
    globalModifier: string;
    globalModifierTooltip: string;
    panicHotkey: string;
    panicHotkeyTooltip: string;
    export: string;
    import: string;
    exportError: (err: string) => string;
    importError: (err: string) => string;
  };
  addPad: {
    title: string;
    audioFile: string;
    audioFileTooltip: string;
    chooseFile: string;
    hotkeyTooltip: string;
    normalizing: string;
    importSuccess: (name: string, lufs: string, gain: string, capped: boolean) => string;
    importError: (err: string) => string;
    importing: string;
    submit: string;
  };
  editPad: {
    title: string;
    volume: (trimDb: string) => string;
    volumeTooltip: string;
    loop: string;
    loopTooltip: string;
    retriggerMode: string;
    retriggerModeTooltip: string;
    fadeIn: string;
    fadeOut: string;
    fadeTooltip: string;
    confirmDelete: string;
    confirmDeleteYes: string;
    delete: string;
  };
  padList: {
    searchPlaceholder: string;
    noResults: string;
    reorderError: (err: string) => string;
  };
  padRow: {
    triggerTitle: string;
    unassigned: string;
    loopShort: string;
  };
  bankTabs: {
    switchRenameTitle: string;
    deleteTitle: string;
    namePlaceholder: string;
    newBankTitle: string;
    newBank: string;
    switchError: (err: string) => string;
    createError: (err: string) => string;
    renameError: (err: string) => string;
    deleteError: (err: string) => string;
    reorderError: (err: string) => string;
  };
  padGrid: {
    empty: string;
  };
  hotkeyPicker: {
    listening: string;
    assigned: (key: string) => string;
    assign: string;
  };
  midiPicker: {
    listening: string;
    assigned: (note: number) => string;
    assign: string;
  };
  trimEditor: {
    title: string;
    trimButton: string;
    analyzing: string;
    analyzeError: (err: string) => string;
    startLabel: string;
    endLabel: string;
    nudgeHint: string;
    play: string;
    pause: string;
    goToStart: string;
    goToEnd: string;
    previewError: (err: string) => string;
    confirm: string;
    invalidNumber: string;
    outOfRange: (minMs: number, maxMs: number) => string;
    startAfterEnd: string;
    endBeforeStart: string;
  };
}

export const en: Dictionary = {
  common: {
    close: "Close",
    cancel: "Cancel",
    save: "Save",
    yes: "Yes",
    no: "No",
    remove: "Remove",
    edit: "Edit",
    done: "Done.",
    name: "Name",
    nameTooltip: "The name shown on this pad in both Config and Live mode. Only affects the label — it does not rename the underlying file.",
    namePlaceholder: "Sound name",
    color: "Color",
    colorTooltip: "Used to tell pads apart at a glance in Live mode: lit up and saturated while playing, dim while stopped.",
    hotkey: "Hot Key",
    hotkeyTooltip: "In Live mode with the app focused, press the key alone. If the app is in the background, use ⌃⌥ (Control+Option) plus the key. Hotkeys only respond on the active bank.",
    hotkeyBackgroundNote: (key) => `${key} — In background: ⌃⌥${key}`,
    midiController: "MIDI Controller",
    midiControllerTooltip: "Click MIDI Learn, then hit the physical pad or key on your controller — the note is captured automatically. Assignments only respond on the active bank.",
    errorPrefix: (err) => `Error: ${err}`,
    triggerError: (err) => `Trigger failed: ${err}`,
    saveError: (err) => `Save failed: ${err}`,
    deleteError: (err) => `Delete failed: ${err}`,
  },
  app: {
    addPad: "Add pad",
    settingsTitle: "Settings",
    config: "Config",
    panic: "Panic",
    panicTitle: "Stops every sound on every bank, immediately",
    live: "Live",
    editMode: "Edit",
    noBanksHint: "Add a pad to create the first bank.",
    deviceLost: (label) =>
      `The output device "${label}" is no longer available. Switched to the default device — check Settings.`,
  },
  settings: {
    title: "Settings",
    outputDevice: "Audio output",
    outputDeviceTooltip: "The device pads play through. Must be different from the background music device so they land on separate mixer channels.",
    testDeviceTitle: "Play a test tone through this device",
    test: "Test",
    playing: "Playing...",
    midiDevice: "MIDI device",
    midiDeviceTooltip: "The MIDI controller the app listens to for triggering pads. Runs in omni mode — it listens on every channel.",
    noMidiDevice: "(no MIDI device)",
    languageTooltip: "Changes the app's interface language. Pad and bank names you typed are never translated — they stay exactly as you wrote them.",
    gridColumns: "Grid columns",
    gridColumnsTooltip: "Number of pad columns shown in the grid. Pads resize automatically to fit the window.",
    targetLufs: "Target LUFS",
    targetLufsTooltip: "The loudness level every pad is normalized to on import. Changing it does not affect existing pads until you click \"Renormalize all\".",
    renormalizeAll: "Renormalize all",
    renormalizing: "Renormalizing all pads...",
    globalHotkeys: "Global hotkeys (⌃⌥+key) enabled",
    globalHotkeysTooltip: "When on, hotkeys still trigger pads even while the app is in the background, using the modifier below plus the key.",
    globalModifier: "Global hotkey modifier",
    globalModifierTooltip: "The modifier held together with a hotkey to trigger pads while the app is in the background. Default is Ctrl+Alt — avoid Cmd+Alt, since macOS reserves several of those combinations.",
    panicHotkey: "Panic button hotkey",
    panicHotkeyTooltip: "A dedicated key that instantly stops every sound on every bank, no matter which bank is active.",
    export: "Export configuration",
    import: "Import configuration",
    exportError: (err) => `Export failed: ${err}`,
    importError: (err) => `Import failed: ${err}`,
  },
  addPad: {
    title: "Add pad",
    audioFile: "Audio file",
    audioFileTooltip: "MP3 or WAV. On import it's automatically resampled to 48kHz and loudness-normalized, so it plays back evenly with the rest of the bank.",
    chooseFile: "Choose file (mp3/wav)",
    hotkeyTooltip: "Optional — you can also assign it later from the edit pad screen. In Live mode with the app focused, press the key alone. In the background, use ⌃⌥ plus the key.",
    normalizing: "Normalizing (LUFS + resample + saving)...",
    importSuccess: (name, lufs, gain, capped) =>
      `Done: "${name}" — measured ${lufs} LUFS, applied gain ${gain}dB` + (capped ? " (capped by true peak)" : ""),
    importError: (err) => `Import failed: ${err}`,
    importing: "Importing...",
    submit: "Add pad",
  },
  editPad: {
    title: "Edit pad",
    volume: (trimDb) => `Volume (trim ${trimDb}dB)`,
    volumeTooltip: "Every sound is automatically normalized on import, so they already play back evenly. This control is just for fine-tuning: raise it if you want this effect to hit harder than the rest.",
    loop: "Loop (repeat playback)",
    loopTooltip: "When on, the pad repeats endlessly until stopped explicitly, by the retrigger mode, or by the panic button.",
    retriggerMode: "Retrigger mode",
    retriggerModeTooltip: "What happens if you trigger the pad while it's already playing. Stop: stops it. Restart: restarts it from the top. Ignore: does nothing. Overlap: layers a new instance on top.",
    fadeIn: "Fade in (ms)",
    fadeOut: "Fade out (ms)",
    fadeTooltip: "Milliseconds of fade in and out. 20ms avoids a click when cutting off. Raise it for music beds that need to come in and out smoothly.",
    confirmDelete: "Delete and remove the audio file?",
    confirmDeleteYes: "Yes, delete",
    delete: "Delete pad",
  },
  padList: {
    searchPlaceholder: "Search by name...",
    noResults: "No results.",
    reorderError: (err) => `Reorder failed: ${err}`,
  },
  padRow: {
    triggerTitle: "Trigger (single button: the retrigger mode decides what happens if it's already playing)",
    unassigned: "Unassigned",
    loopShort: "loop",
  },
  bankTabs: {
    switchRenameTitle: "Click: switch bank. Double-click: rename.",
    deleteTitle: "Delete bank (also deletes its pads)",
    namePlaceholder: "Bank name",
    newBankTitle: "New bank",
    newBank: "Bank",
    switchError: (err) => `Switch bank failed: ${err}`,
    createError: (err) => `Create bank failed: ${err}`,
    renameError: (err) => `Rename failed: ${err}`,
    deleteError: (err) => `Delete bank failed: ${err}`,
    reorderError: (err) => `Reorder banks failed: ${err}`,
  },
  padGrid: {
    empty: "No pads yet.",
  },
  hotkeyPicker: {
    listening: "Press A-Z or 0-9... (Esc cancels)",
    assigned: (key) => `Key: ${key}`,
    assign: "Assign key",
  },
  midiPicker: {
    listening: "Hit the physical pad... (click cancels)",
    assigned: (note) => `MIDI note: ${note}`,
    assign: "MIDI learn",
  },
  trimEditor: {
    title: "Trim the clip",
    trimButton: "Trim",
    analyzing: "Loading waveform...",
    analyzeError: (err) => `Couldn't load the waveform: ${err}`,
    startLabel: "Start (ms)",
    endLabel: "End (ms)",
    nudgeHint: "Arrow keys nudge the selected handle by 10ms (Shift: 100ms).",
    play: "Play selection",
    pause: "Pause",
    goToStart: "Jump to start",
    goToEnd: "Jump to end",
    previewError: (err) => `Preview failed: ${err}`,
    confirm: "Use this trim",
    invalidNumber: "Enter a valid number of milliseconds.",
    outOfRange: (minMs, maxMs) => `Must be between ${minMs} and ${maxMs}ms.`,
    startAfterEnd: "Start must be at least 50ms before the end.",
    endBeforeStart: "End must be at least 50ms after the start.",
  },
};

export const es: Dictionary = {
  common: {
    close: "Cerrar",
    cancel: "Cancelar",
    save: "Guardar",
    yes: "Sí",
    no: "No",
    remove: "Quitar",
    edit: "Editar",
    done: "Listo.",
    name: "Nombre",
    nameTooltip: "El nombre que se muestra en este pad, tanto en modo Config como en Live. Solo afecta la etiqueta — no renombra el archivo de audio.",
    namePlaceholder: "Nombre del sonido",
    color: "Color",
    colorTooltip: "Sirve para distinguir los pads de un vistazo en modo Live: encendido y saturado mientras suena, apagado cuando está detenido.",
    hotkey: "Hot Key",
    hotkeyTooltip: "En modo Live con la app en primer plano, presiona la tecla sola. Si la app está en segundo plano, usa ⌃⌥ (Control+Option) más la tecla. Los atajos solo responden al banco activo.",
    hotkeyBackgroundNote: (key) => `${key} — En segundo plano: ⌃⌥${key}`,
    midiController: "Controlador MIDI",
    midiControllerTooltip: "Hacé clic en MIDI learn y después tocá el pad o tecla físico del controlador — la nota se captura sola. Las asignaciones solo responden al banco activo.",
    errorPrefix: (err) => `Error: ${err}`,
    triggerError: (err) => `Error al disparar: ${err}`,
    saveError: (err) => `Error al guardar: ${err}`,
    deleteError: (err) => `Error al eliminar: ${err}`,
  },
  app: {
    addPad: "Agregar pad",
    settingsTitle: "Configuración",
    config: "Config",
    panic: "Pánico",
    panicTitle: "Detiene todos los sonidos de todos los bancos, de inmediato",
    live: "Live",
    editMode: "Editar",
    noBanksHint: "Agregá un pad para crear el primer banco.",
    deviceLost: (label) =>
      `El dispositivo de salida "${label}" ya no está disponible. Se cambió al predeterminado — revisá Configuración.`,
  },
  settings: {
    title: "Configuración",
    outputDevice: "Salida de audio",
    outputDeviceTooltip: "Dispositivo por donde salen los pads. Debe ser distinto al de la música de fondo para que caigan en canales separados de la mixer.",
    testDeviceTitle: "Reproducir un tono de prueba por este dispositivo",
    test: "Test",
    playing: "Reproduciendo...",
    midiDevice: "Dispositivo MIDI",
    midiDeviceTooltip: "El controlador MIDI que escucha la app para disparar pads. Funciona en modo omni — escucha en todos los canales.",
    noMidiDevice: "(sin dispositivo MIDI)",
    languageTooltip: "Cambia el idioma de la interfaz de la app. Los nombres de pads y bancos que escribiste nunca se traducen — se respetan tal cual los pusiste.",
    gridColumns: "Columnas del grid",
    gridColumnsTooltip: "Cantidad de columnas de pads que se muestran en el grid. Los pads se ajustan de tamaño automáticamente para caber en la ventana.",
    targetLufs: "Target LUFS",
    targetLufsTooltip: "El nivel de sonoridad al que se normaliza cada pad al importarlo. Cambiarlo no afecta a los pads existentes hasta que hagas clic en \"Renormalizar todo\".",
    renormalizeAll: "Renormalizar todo",
    renormalizing: "Renormalizando todos los pads...",
    globalHotkeys: "Atajos globales (⌃⌥+tecla) activados",
    globalHotkeysTooltip: "Con esto activado, los atajos siguen disparando pads aunque la app esté en segundo plano, usando el modificador de abajo más la tecla.",
    globalModifier: "Modificador de atajos globales",
    globalModifierTooltip: "El modificador que se mantiene apretado junto con la tecla para disparar pads con la app en segundo plano. Por defecto es Ctrl+Alt — evitá Cmd+Alt, porque macOS reserva varias de esas combinaciones.",
    panicHotkey: "Tecla del botón de pánico",
    panicHotkeyTooltip: "Una tecla dedicada que detiene al instante todos los sonidos de todos los bancos, sin importar cuál esté activo.",
    export: "Exportar configuración",
    import: "Importar configuración",
    exportError: (err) => `Error al exportar: ${err}`,
    importError: (err) => `Error al importar: ${err}`,
  },
  addPad: {
    title: "Agregar pad",
    audioFile: "Archivo de audio",
    audioFileTooltip: "MP3 o WAV. Al importarlo se resamplea a 48kHz y se normaliza el volumen automáticamente, así suena parejo con el resto del banco.",
    chooseFile: "Elegir archivo (mp3/wav)",
    hotkeyTooltip: "Opcional — también podés asignarla después desde la pantalla de editar pad. En modo Live con la app en primer plano, presioná la tecla sola. En segundo plano, usá ⌃⌥ más la tecla.",
    normalizing: "Normalizando (LUFS + resample + guardado)...",
    importSuccess: (name, lufs, gain, capped) =>
      `Listo: "${name}" — ${lufs} LUFS medidos, ganancia aplicada ${gain}dB` + (capped ? " (recortada por true peak)" : ""),
    importError: (err) => `Error al importar: ${err}`,
    importing: "Importando...",
    submit: "Agregar pad",
  },
  editPad: {
    title: "Editar pad",
    volume: (trimDb) => `Volumen (trim ${trimDb}dB)`,
    volumeTooltip: "Todos los audios se normalizan automáticamente al importarlos, así que ya suenan parejos. Este control es solo para ajuste fino: súbelo si quieres que este efecto pegue más fuerte que el resto.",
    loop: "Loop (reproducción en bucle)",
    loopTooltip: "Con esto activado, el pad se repite sin parar hasta que lo detengas manualmente, por el modo de re-disparo, o con el botón de pánico.",
    retriggerMode: "Modo de re-disparo",
    retriggerModeTooltip: "Qué pasa si disparas el pad mientras ya está sonando. Stop: lo detiene. Restart: lo reinicia desde el principio. Ignore: no hace nada. Overlap: lo encima.",
    fadeIn: "Fade in (ms)",
    fadeOut: "Fade out (ms)",
    fadeTooltip: "Milisegundos de entrada y salida. 20ms evita el chasquido al cortar. Súbelo para camas musicales que deban entrar y salir suave.",
    confirmDelete: "¿Eliminar y borrar el audio?",
    confirmDeleteYes: "Sí, eliminar",
    delete: "Eliminar pad",
  },
  padList: {
    searchPlaceholder: "Buscar por nombre...",
    noResults: "Sin resultados.",
    reorderError: (err) => `Error al reordenar: ${err}`,
  },
  padRow: {
    triggerTitle: "Disparar (un solo botón: el modo de re-disparo decide qué pasa si ya está sonando)",
    unassigned: "Sin asignar",
    loopShort: "loop",
  },
  bankTabs: {
    switchRenameTitle: "Clic: cambiar de banco. Doble clic: renombrar.",
    deleteTitle: "Eliminar banco (borra también sus pads)",
    namePlaceholder: "Nombre del banco",
    newBankTitle: "Nuevo banco",
    newBank: "Banco",
    switchError: (err) => `Error al cambiar de banco: ${err}`,
    createError: (err) => `Error al crear banco: ${err}`,
    renameError: (err) => `Error al renombrar: ${err}`,
    deleteError: (err) => `Error al eliminar banco: ${err}`,
    reorderError: (err) => `Error al reordenar bancos: ${err}`,
  },
  padGrid: {
    empty: "Sin pads todavía.",
  },
  hotkeyPicker: {
    listening: "Presioná A-Z o 0-9... (Esc cancela)",
    assigned: (key) => `Tecla: ${key}`,
    assign: "Asignar tecla",
  },
  midiPicker: {
    listening: "Tocá el pad físico... (clic cancela)",
    assigned: (note) => `Nota MIDI: ${note}`,
    assign: "MIDI learn",
  },
  trimEditor: {
    title: "Recortar el audio",
    trimButton: "Recortar",
    analyzing: "Cargando forma de onda...",
    analyzeError: (err) => `No se pudo cargar la forma de onda: ${err}`,
    startLabel: "Inicio (ms)",
    endLabel: "Fin (ms)",
    nudgeHint: "Las flechas mueven el handle seleccionado 10ms (Shift: 100ms).",
    play: "Reproducir selección",
    pause: "Pausar",
    goToStart: "Ir al inicio",
    goToEnd: "Ir al final",
    previewError: (err) => `Error al reproducir: ${err}`,
    confirm: "Usar este recorte",
    invalidNumber: "Ingresá un número válido de milisegundos.",
    outOfRange: (minMs, maxMs) => `Debe estar entre ${minMs} y ${maxMs}ms.`,
    startAfterEnd: "El inicio debe quedar al menos 50ms antes del fin.",
    endBeforeStart: "El fin debe quedar al menos 50ms después del inicio.",
  },
};
