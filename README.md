# Sound Pad

Sound pad de escritorio para producción de podcast/streaming en vivo (macOS). Bancos de pads con sonidos, atajos de teclado globales, control MIDI y normalización de volumen automática (LUFS).

## Instalación

1. Andá a [Releases](https://github.com/zinetnorf/sound-pad/releases) y descargá el `.dmg` más reciente (`Sound.Pad_x.y.z_universal.dmg`).
2. Abrí el `.dmg` y arrastrá **Sound Pad** a la carpeta **Applications**.
3. La app no está firmada ni notarizada por Apple, así que en el primer arranque macOS (Gatekeeper) va a bloquearla con un aviso de "app dañada" o "no se puede verificar el desarrollador". Para abrirla:
   - Click derecho sobre **Sound Pad.app** → **Abrir** → confirmar en el diálogo, o
   - Si el aviso dice "dañada", corré en Terminal: `xattr -cr /Applications/Sound\ Pad.app` y volvé a abrirla.
4. En el primer arranque la app crea automáticamente un banco **Default** con 9 sonidos de ejemplo (risas, aplausos, abucheo, drum roll, ta-da, wa-wa-wa, ba-dum-tss), listos para usar.

## Uso

- **Reproducir un pad:** click sobre el pad, o su atajo de teclado si tiene uno asignado.
- **Botón de pánico:** detiene todo lo que esté sonando de inmediato (con un fade-out corto para evitar el chasquido).
- **Agregar un sonido:** modo Configuración → "Agregar pad" → elegís un archivo MP3/WAV, nombre, color, atajo y/o nota MIDI. La app lo normaliza y convierte automáticamente.
- **Editar un pad:** volumen (trim), loop, modo de re-disparo (parar/reiniciar/ignorar/superponer), fades de entrada/salida, atajo, nota MIDI, color.
- **Bancos:** organizá pads por banco (por ejemplo, uno por programa). Los atajos y notas MIDI son por banco, así podés reutilizar las mismas teclas en cada uno.
- **Dispositivo de salida y MIDI:** configurables en modo Configuración; si el dispositivo de salida desaparece, la app cae al dispositivo por defecto y avisa.
- **Exportar/Importar:** modo Configuración → exportá toda tu configuración y audios a un `.zip`, o importá uno para restaurar/migrar a otra máquina.
- **Renormalizar todo:** recalcula el volumen de todos los pads si cambiás el target de LUFS.

## Sección técnica

**Stack:** [Tauri v2](https://tauri.app/) (backend en Rust + WebView nativo) con frontend en React 19 + TypeScript, Vite y Tailwind CSS v4. Sin backend remoto: todo corre local.

**Persistencia:** no hay base de datos. El estado vive en `config.json` (bancos, pads, configuración) dentro del directorio de datos de la app (`~/Library/Application Support/com.soundpad.app/`), con escritura atómica (archivo temporal + rename) para no dejarlo corrupto ante un corte. El audio normalizado de cada pad se guarda como `audio/{pad_uuid}.wav` en esa misma carpeta.

**Pipeline de importación de audio** (`src-tauri/src/audio/normalize.rs`), el mismo que se usa tanto al importar un pad manualmente como al sembrar el banco Default en el primer arranque:

1. Decodificar el archivo fuente (MP3/WAV) con [`symphonia`](https://github.com/pdeljanov/Symphonia).
2. Resamplear a 48 kHz con [`rubato`](https://github.com/HEnquist/rubato) (sinc de alta calidad, offline).
3. Convertir a estéreo (mono se duplica a L/R; más de 2 canales se recorta a los primeros dos).
4. Medir loudness integrado (o en ventana, para clips cortos) y true peak con [`ebur128`](https://github.com/sdroege/ebur128) (BS.1770).
5. Calcular la ganancia necesaria para llegar al LUFS objetivo, recortada si excede el techo de true peak (-1 dBTP) para evitar distorsión.
6. Escribir el resultado como WAV PCM float de 32 bits, 48 kHz estéreo, con [`hound`](https://github.com/ruuda/hound).

**Reproducción:** motor de audio persistente sobre [`kira`](https://github.com/tesselode/kira) + [`cpal`](https://github.com/RustAudio/cpal), con precarga del banco activo, modos de re-disparo, fades configurables y hot-swap de dispositivo de salida sin reiniciar la app.

**Atajos y MIDI:** atajos globales de teclado (`tauri-plugin-global-shortcut`) y entrada MIDI (`midir`) resueltos por banco activo — el mismo mapa de teclas/notas se puede reutilizar en distintos bancos sin choques.

**Arquitectura del backend:** comandos de Tauri (`src-tauri/src/commands.rs`) como única puerta de entrada del frontend al estado; módulos separados por responsabilidad (`audio/`, `models.rs`, `storage.rs`, `hotkeys.rs`, `midi.rs`, `export.rs`, `seed.rs`). Cobertura de tests unitarios en Rust para el pipeline de normalización y las operaciones sobre bancos/pads.

**CI/CD:** al pushear a `main`, un workflow de GitHub Actions (`.github/workflows/release.yml`) compila la app para macOS (universal: Apple Silicon + Intel) con [`tauri-action`](https://github.com/tauri-apps/tauri-action) y publica el `.dmg` resultante como release en borrador en la sección [Releases](https://github.com/zinetnorf/sound-pad/releases).

## Desarrollo local

```bash
pnpm install
pnpm tauri dev    # levanta la app en modo desarrollo
pnpm tauri build  # genera el instalador de producción localmente
```

Requiere [Rust](https://www.rust-lang.org/tools/install), [pnpm](https://pnpm.io/) y las dependencias nativas de Tauri para macOS (ver [prerequisitos de Tauri](https://tauri.app/start/prerequisites/)).
