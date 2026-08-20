import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import WaveSurfer from "wavesurfer.js";
import { Loader2, Pause, Play, SkipBack, SkipForward, X } from "lucide-react";
import { useT } from "../../i18n/LanguageContext";

interface SourceAnalysis {
  durationMs: number;
  // Un array por canal (L, R): pico con signo de mayor magnitud por bucket,
  // calculado en Rust — nunca se decodifica el archivo en el WebView
  // (SPEC-recorte-v2.md "Forma de onda").
  peaks: number[][];
  sourceSampleRate: number;
  sourceChannels: number;
}

interface SnappedRegion {
  startMs: number;
  endMs: number;
}

interface TrimEditorProps {
  sourcePath: string;
  onCancel: () => void;
  onConfirm: (trim: { startMs: number; endMs: number }) => void;
}

const MIN_REGION_MS = 50;
const NUDGE_MS = 10;
const NUDGE_MS_SHIFT = 100;
/// Cuánto se escucha al auditar un borde con los botones de ir al
/// inicio/final — no todo el clip, solo el punto de corte.
const EDGE_AUDITION_MS = 500;

/// Editor de recorte del modal "Agregar Pad" (SPEC-recorte-v2.md). La
/// reproducción NUNCA pasa por el WebView (PLAN.md §2: WKWebView no tiene
/// AudioContext.setSinkId) — wavesurfer.js solo dibuja la forma de onda
/// (modo "silent": peaks + duration, sin url) y el preview real lo hace
/// Rust por el mismo device de los pads vía el comando `preview_region`.
///
/// La región seleccionable es un overlay propio (divs + Pointer Events), no
/// el plugin de regiones de wavesurfer: ese plugin renderiza sus handles
/// dentro de un shadow root cerrado a estilos normales (border negro sobre
/// fondo negro, prácticamente invisible) y no daba control fino sobre el
/// look "afuera atenuado / adentro claro" estilo QuickTime que pidió el
/// usuario. Con overlay propio se controla el estilo y el drag por completo.
function TrimEditor({ sourcePath, onCancel, onConfirm }: TrimEditorProps) {
  const t = useT();
  const modalRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const waveformBoxRef = useRef<HTMLDivElement>(null);
  const wsRef = useRef<WaveSurfer | null>(null);
  const playTimeoutRef = useRef<number | undefined>(undefined);
  const cursorRafRef = useRef<number | undefined>(undefined);
  // Reloj propio para animar el cursor: wavesurfer está en modo "silent"
  // (sin audio real), así que su propio playhead nunca avanza solo — el
  // audio de verdad lo reproduce Rust, hay que mover la línea a mano.
  const previewClockRef = useRef<{ startWallClockMs: number; startAudioS: number } | null>(null);

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [durationMs, setDurationMs] = useState(0);
  const [startMs, setStartMs] = useState(0);
  const [endMs, setEndMs] = useState(0);
  const [startInput, setStartInput] = useState("0");
  const [endInput, setEndInput] = useState("0");
  const [startError, setStartError] = useState("");
  const [endError, setEndError] = useState("");
  const [playing, setPlaying] = useState(false);
  const [activeHandle, setActiveHandle] = useState<"start" | "end">("end");

  // Pide a Rust el punto de cruce por cero más cercano a cada borde
  // (SPEC-recorte-v2.md "Snap a cruce por cero") — el frontend nunca decide
  // el snap por su cuenta, Rust es la única fuente de verdad. Se llama SOLO
  // al soltar el mouse o confirmar un campo numérico, nunca en cada pixel
  // de un drag en curso.
  const applySnap = useCallback(async (rawStartMs: number, rawEndMs: number) => {
    try {
      const snapped = await invoke<SnappedRegion>("snap_region", { startMs: rawStartMs, endMs: rawEndMs });
      setStartMs(snapped.startMs);
      setEndMs(snapped.endMs);
      setStartInput(String(snapped.startMs));
      setEndInput(String(snapped.endMs));
    } catch {
      // El snap es una mejora, no algo crítico para poder seguir editando.
      setStartMs(rawStartMs);
      setEndMs(rawEndMs);
      setStartInput(String(rawStartMs));
      setEndInput(String(rawEndMs));
    }
  }, []);

  // El diálogo nativo de "Elegir archivo" (@tauri-apps/plugin-dialog) le
  // saca el foco de teclado al WebView. Si no se reclama, el listener de
  // flechas de más abajo queda suscrito pero nunca recibe eventos.
  useEffect(() => {
    modalRef.current?.focus();
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError("");

    invoke<SourceAnalysis>("analyze_source", { sourcePath })
      .then((analysis) => {
        if (cancelled || !containerRef.current) return;

        setDurationMs(analysis.durationMs);
        setStartMs(0);
        setEndMs(analysis.durationMs);
        setStartInput("0");
        setEndInput(String(analysis.durationMs));

        const ws = WaveSurfer.create({
          container: containerRef.current,
          height: 96,
          waveColor: "#7c4dff",
          progressColor: "#7c4dff",
          cursorColor: "#f5f5f5",
          cursorWidth: 2,
          interact: false,
          peaks: analysis.peaks,
          duration: analysis.durationMs / 1000,
        });
        wsRef.current = ws;

        setLoading(false);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(t.trimEditor.analyzeError(`${err}`));
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
      wsRef.current?.destroy();
      wsRef.current = null;
      if (playTimeoutRef.current) window.clearTimeout(playTimeoutRef.current);
      if (cursorRafRef.current) cancelAnimationFrame(cursorRafRef.current);
      // El cache de `analyze_source` NO se limpia acá: si el usuario confirma
      // el recorte, `import_pad` todavía lo necesita para no volver a
      // decodificar el archivo. Quien decide cuándo liberarlo del todo es
      // AddPadModal (dueño del ciclo de vida completo del import).
      invoke("stop_preview").catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourcePath]);

  // Nudge con flechas del teclado sobre el handle activo (SPEC-recorte-v2.md
  // "UI del editor"). Izquierda/derecha para no chocar con el spinner nativo
  // de <input type=number>, que ya usa arriba/abajo.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
      e.preventDefault();
      const delta = (e.shiftKey ? NUDGE_MS_SHIFT : NUDGE_MS) * (e.key === "ArrowLeft" ? -1 : 1);
      if (activeHandle === "start") {
        void applySnap(Math.max(0, Math.min(startMs + delta, endMs - MIN_REGION_MS)), endMs);
      } else {
        void applySnap(startMs, Math.min(durationMs, Math.max(endMs + delta, startMs + MIN_REGION_MS)));
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeHandle, startMs, endMs, durationMs, applySnap]);

  function clientXToMs(clientX: number, rect: DOMRect) {
    const ratio = Math.min(Math.max((clientX - rect.left) / rect.width, 0), 1);
    return Math.round(ratio * durationMs);
  }

  // Arrastrar el handle izquierdo o derecho redefine ese borde solo,
  // igual que QuickTime: el otro borde queda fijo durante todo el drag.
  function handleHandlePointerDown(which: "start" | "end", e: React.PointerEvent) {
    e.preventDefault();
    e.stopPropagation();
    const box = waveformBoxRef.current;
    if (!box || durationMs <= 0) return;
    const rect = box.getBoundingClientRect();
    const fixedStart = startMs;
    const fixedEnd = endMs;
    setActiveHandle(which);

    function onMove(ev: PointerEvent) {
      const ms = clientXToMs(ev.clientX, rect);
      if (which === "start") {
        const clamped = Math.max(0, Math.min(ms, fixedEnd - MIN_REGION_MS));
        setStartMs(clamped);
        setStartInput(String(clamped));
      } else {
        const clamped = Math.min(durationMs, Math.max(ms, fixedStart + MIN_REGION_MS));
        setEndMs(clamped);
        setEndInput(String(clamped));
      }
    }
    function onUp(ev: PointerEvent) {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      const ms = clientXToMs(ev.clientX, rect);
      const finalStart = which === "start" ? Math.max(0, Math.min(ms, fixedEnd - MIN_REGION_MS)) : fixedStart;
      const finalEnd = which === "end" ? Math.min(durationMs, Math.max(ms, fixedStart + MIN_REGION_MS)) : fixedEnd;
      void applySnap(finalStart, finalEnd);
    }
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  // Arrastrar dentro de la selección mueve los dos bordes juntos, sin
  // cambiar el largo — para reubicar la ventana ya elegida sin tener que
  // re-ajustar ambos handles a mano.
  function handleBoxPointerDown(e: React.PointerEvent) {
    e.preventDefault();
    const box = waveformBoxRef.current;
    if (!box || durationMs <= 0) return;
    const rect = box.getBoundingClientRect();
    const fixedStart = startMs;
    const regionLen = endMs - startMs;
    const originMs = clientXToMs(e.clientX, rect);

    function computeNewStart(clientX: number) {
      const deltaMs = clientXToMs(clientX, rect) - originMs;
      return Math.max(0, Math.min(fixedStart + deltaMs, durationMs - regionLen));
    }
    function onMove(ev: PointerEvent) {
      const newStart = computeNewStart(ev.clientX);
      setStartMs(newStart);
      setEndMs(newStart + regionLen);
      setStartInput(String(newStart));
      setEndInput(String(newStart + regionLen));
    }
    function onUp(ev: PointerEvent) {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      const newStart = computeNewStart(ev.clientX);
      void applySnap(newStart, newStart + regionLen);
    }
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  // wavesurfer en modo "silent" no tiene audio propio, así que su cursor
  // nunca avanza solo — se anima a mano contra un reloj de pared, en sync
  // con lo que Rust está reproduciendo de verdad.
  function animateCursor() {
    const clock = previewClockRef.current;
    if (!clock || !wsRef.current) return;
    const elapsedS = (performance.now() - clock.startWallClockMs) / 1000;
    wsRef.current.setTime(clock.startAudioS + elapsedS);
    cursorRafRef.current = requestAnimationFrame(animateCursor);
  }

  function stopCursorAnimation(resetToS?: number) {
    if (cursorRafRef.current) cancelAnimationFrame(cursorRafRef.current);
    cursorRafRef.current = undefined;
    previewClockRef.current = null;
    if (resetToS !== undefined) wsRef.current?.setTime(resetToS);
  }

  async function playSelection() {
    setError("");
    try {
      await invoke("preview_region", { startMs, endMs });
      setPlaying(true);
      previewClockRef.current = { startWallClockMs: performance.now(), startAudioS: startMs / 1000 };
      if (cursorRafRef.current) cancelAnimationFrame(cursorRafRef.current);
      cursorRafRef.current = requestAnimationFrame(animateCursor);
      if (playTimeoutRef.current) window.clearTimeout(playTimeoutRef.current);
      playTimeoutRef.current = window.setTimeout(() => {
        setPlaying(false);
        stopCursorAnimation(startMs / 1000);
      }, Math.max(endMs - startMs, 0));
    } catch (err) {
      setError(t.trimEditor.previewError(`${err}`));
    }
  }

  async function pausePreview() {
    if (playTimeoutRef.current) window.clearTimeout(playTimeoutRef.current);
    setPlaying(false);
    stopCursorAnimation(startMs / 1000);
    await invoke("stop_preview").catch(() => {});
  }

  // Escuchar solo el punto de corte (no el clip entero) es lo que de verdad
  // ayuda a decidir si el recorte quedó bien puesto.
  async function auditionEdge(edge: "start" | "end") {
    setError("");
    const from = edge === "start" ? startMs : Math.max(startMs, endMs - EDGE_AUDITION_MS);
    const to = edge === "start" ? Math.min(endMs, startMs + EDGE_AUDITION_MS) : endMs;
    try {
      await invoke("preview_region", { startMs: from, endMs: to });
      previewClockRef.current = { startWallClockMs: performance.now(), startAudioS: from / 1000 };
      if (cursorRafRef.current) cancelAnimationFrame(cursorRafRef.current);
      cursorRafRef.current = requestAnimationFrame(animateCursor);
      if (playTimeoutRef.current) window.clearTimeout(playTimeoutRef.current);
      playTimeoutRef.current = window.setTimeout(() => stopCursorAnimation(startMs / 1000), Math.max(to - from, 0));
    } catch (err) {
      setError(t.trimEditor.previewError(`${err}`));
    }
  }

  // Valida el texto crudo del campo: número real, dentro de [0, duration],
  // y que deje al menos MIN_REGION_MS de separación con el otro borde. El
  // valor aplicado siempre queda clampeado a algo válido — el mensaje solo
  // explica por qué lo que escribiste no fue exactamente lo que quedó.
  function commitStartInput(raw: string) {
    setStartInput(raw);
    const parsed = Math.round(Number(raw));
    if (raw.trim() === "" || !Number.isFinite(parsed)) {
      setStartError(t.trimEditor.invalidNumber);
      return;
    }
    if (parsed < 0 || parsed > durationMs) {
      setStartError(t.trimEditor.outOfRange(0, durationMs));
      void applySnap(Math.max(0, Math.min(parsed, durationMs, endMs - MIN_REGION_MS)), endMs);
      return;
    }
    if (parsed > endMs - MIN_REGION_MS) {
      setStartError(t.trimEditor.startAfterEnd);
      void applySnap(Math.max(0, endMs - MIN_REGION_MS), endMs);
      return;
    }
    setStartError("");
    void applySnap(parsed, endMs);
  }

  function commitEndInput(raw: string) {
    setEndInput(raw);
    const parsed = Math.round(Number(raw));
    if (raw.trim() === "" || !Number.isFinite(parsed)) {
      setEndError(t.trimEditor.invalidNumber);
      return;
    }
    if (parsed < 0 || parsed > durationMs) {
      setEndError(t.trimEditor.outOfRange(0, durationMs));
      void applySnap(startMs, Math.max(startMs + MIN_REGION_MS, Math.min(parsed, durationMs)));
      return;
    }
    if (parsed < startMs + MIN_REGION_MS) {
      setEndError(t.trimEditor.endBeforeStart);
      void applySnap(startMs, Math.min(durationMs, startMs + MIN_REGION_MS));
      return;
    }
    setEndError("");
    void applySnap(startMs, parsed);
  }

  const startPercent = durationMs > 0 ? (startMs / durationMs) * 100 : 0;
  const endPercent = durationMs > 0 ? (endMs / durationMs) * 100 : 100;

  return (
    <div className="fixed inset-0 z-20 flex items-center justify-center bg-black/60 p-4">
      <div
        ref={modalRef}
        tabIndex={-1}
        className="flex max-h-[90vh] w-full max-w-2xl flex-col gap-4 overflow-y-auto rounded-xl bg-neutral-900 p-6 shadow-2xl outline-none"
      >
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t.trimEditor.title}</h2>
          <button
            className="rounded-full p-1.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
            onClick={onCancel}
            title={t.common.close}
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {error && <p className="text-sm text-red-400">{error}</p>}

        {/* Alto fijo (96px = altura de wavesurfer) para que la caja no
            colapse a 0px mientras carga. */}
        <div ref={waveformBoxRef} className="relative h-24 w-full select-none overflow-hidden rounded bg-neutral-950">
          {loading && (
            <div className="absolute inset-0 z-10 flex items-center justify-center gap-2 text-sm text-neutral-400">
              <Loader2 className="h-4 w-4 shrink-0 animate-spin" />
              {t.trimEditor.analyzing}
            </div>
          )}
          <div ref={containerRef} className="h-full w-full" />

          {!loading && durationMs > 0 && (
            <>
              {/* Afuera de la selección, atenuado — igual que QuickTime. */}
              <div className="pointer-events-none absolute inset-y-0 left-0 bg-black/65" style={{ width: `${startPercent}%` }} />
              <div className="pointer-events-none absolute inset-y-0 right-0 bg-black/65" style={{ width: `${100 - endPercent}%` }} />

              {/* Ventana seleccionada: arrastrable como bloque para mover
                  ambos bordes juntos sin cambiar el largo. */}
              <div
                onPointerDown={handleBoxPointerDown}
                className="absolute inset-y-0 cursor-grab border-y-2 border-accent/80 active:cursor-grabbing"
                style={{ left: `${startPercent}%`, width: `${endPercent - startPercent}%` }}
              />

              {/* Handle izquierdo: redefine el inicio. */}
              <div
                onPointerDown={(e) => handleHandlePointerDown("start", e)}
                className="absolute inset-y-0 z-10 flex w-4 -translate-x-1/2 cursor-ew-resize touch-none items-center justify-center"
                style={{ left: `${startPercent}%` }}
              >
                <div className="h-full w-1.5 rounded bg-accent shadow-[0_0_4px_rgba(0,0,0,0.6)]" />
              </div>

              {/* Handle derecho: redefine el final. */}
              <div
                onPointerDown={(e) => handleHandlePointerDown("end", e)}
                className="absolute inset-y-0 z-10 flex w-4 -translate-x-1/2 cursor-ew-resize touch-none items-center justify-center"
                style={{ left: `${endPercent}%` }}
              >
                <div className="h-full w-1.5 rounded bg-accent shadow-[0_0_4px_rgba(0,0,0,0.6)]" />
              </div>
            </>
          )}
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => auditionEdge("start")}
            title={t.trimEditor.goToStart}
            disabled={loading}
            className="rounded bg-neutral-800 p-2 hover:bg-neutral-700 disabled:opacity-40"
          >
            <SkipBack className="h-4 w-4" />
          </button>
          {playing ? (
            <button
              type="button"
              onClick={pausePreview}
              className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-2 text-sm hover:bg-neutral-700"
            >
              <Pause className="h-4 w-4" />
              {t.trimEditor.pause}
            </button>
          ) : (
            <button
              type="button"
              onClick={playSelection}
              disabled={loading}
              className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-2 text-sm hover:bg-neutral-700 disabled:opacity-40"
            >
              <Play className="h-4 w-4" />
              {t.trimEditor.play}
            </button>
          )}
          <button
            type="button"
            onClick={() => auditionEdge("end")}
            title={t.trimEditor.goToEnd}
            disabled={loading}
            className="rounded bg-neutral-800 p-2 hover:bg-neutral-700 disabled:opacity-40"
          >
            <SkipForward className="h-4 w-4" />
          </button>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <label className="flex flex-col gap-1 text-sm">
            <span>{t.trimEditor.startLabel}</span>
            <input
              type="number"
              min={0}
              max={Math.max(0, endMs - MIN_REGION_MS)}
              step={1}
              value={startInput}
              disabled={loading}
              onFocus={() => setActiveHandle("start")}
              onChange={(e) => commitStartInput(e.currentTarget.value)}
              className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
            />
            {startError && <span className="text-xs text-red-400">{startError}</span>}
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span>{t.trimEditor.endLabel}</span>
            <input
              type="number"
              min={startMs + MIN_REGION_MS}
              max={durationMs}
              step={1}
              value={endInput}
              disabled={loading}
              onFocus={() => setActiveHandle("end")}
              onChange={(e) => commitEndInput(e.currentTarget.value)}
              className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
            />
            {endError && <span className="text-xs text-red-400">{endError}</span>}
          </label>
        </div>

        <p className="text-xs text-neutral-500">{t.trimEditor.nudgeHint}</p>

        <div className="flex justify-end gap-2 border-t border-neutral-800 pt-4">
          <button className="rounded bg-neutral-800 px-4 py-2 font-medium hover:bg-neutral-700" onClick={onCancel}>
            {t.common.cancel}
          </button>
          <button
            className="rounded bg-gradient-to-r from-primary to-accent px-4 py-2 font-medium text-white hover:opacity-90 disabled:opacity-40"
            disabled={loading || !!error || !!startError || !!endError}
            onClick={() => onConfirm({ startMs, endMs })}
          >
            {t.trimEditor.confirm}
          </button>
        </div>
      </div>
    </div>
  );
}

export default TrimEditor;
