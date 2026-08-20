import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FileAudio, Loader2, Scissors, X } from "lucide-react";
import type { Pad } from "../../types/models";
import HelpTooltip from "../shared/HelpTooltip";
import HotkeyPicker from "../shared/HotkeyPicker";
import MidiLearnPicker from "../shared/MidiLearnPicker";
import TrimEditor from "./TrimEditor";
import { useT } from "../../i18n/LanguageContext";

const DEFAULT_COLORS = ["#E8734A", "#4AA3E8", "#7CE84A", "#E8D24A", "#B14AE8"];

interface AddPadModalProps {
  onImported: () => void;
  onClose: () => void;
}

interface TrimSelection {
  startMs: number;
  endMs: number;
}

function formatMs(ms: number) {
  const totalTenths = Math.round(ms / 100);
  const seconds = Math.floor(totalTenths / 10);
  const tenths = totalTenths % 10;
  return `${seconds}.${tenths}s`;
}

function AddPadModal({ onImported, onClose }: AddPadModalProps) {
  const t = useT();
  const [sourcePath, setSourcePath] = useState("");
  const [name, setName] = useState("");
  const [color, setColor] = useState(DEFAULT_COLORS[0]);
  const [hotkey, setHotkey] = useState<string | null>(null);
  const [midiNote, setMidiNote] = useState<number | null>(null);
  const [status, setStatus] = useState("");
  const [importing, setImporting] = useState(false);
  // El recorte es destructivo y opcional (SPEC-recorte-v2.md): `null` significa
  // "usar el archivo completo", igual que el comportamiento de v1.
  const [trim, setTrim] = useState<TrimSelection | null>(null);
  const [showTrimEditor, setShowTrimEditor] = useState(false);

  async function releaseSourceCache() {
    await invoke("clear_source_cache").catch(() => {});
  }

  async function pickFile() {
    const path = await open({
      multiple: false,
      filters: [{ name: "Audio", extensions: ["mp3", "wav"] }],
    });
    if (typeof path === "string") {
      setSourcePath(path);
      setTrim(null);
      if (!name) {
        const base = path.split("/").pop() ?? path;
        setName(base.replace(/\.(mp3|wav)$/i, ""));
      }
    }
  }

  function handleTrimConfirm(selection: TrimSelection) {
    setTrim(selection);
    setShowTrimEditor(false);
  }

  async function handleTrimCancel() {
    // Cancelar el editor no cancela el pad entero: se sigue con el archivo
    // completo, igual que si el usuario nunca hubiera abierto el recorte.
    setTrim(null);
    setShowTrimEditor(false);
    await releaseSourceCache();
  }

  async function handleClose() {
    await releaseSourceCache();
    onClose();
  }

  async function handleImport() {
    if (!sourcePath || !name) return;
    setImporting(true);
    setStatus(t.addPad.normalizing);
    try {
      const pad = await invoke<Pad>("import_pad", {
        sourcePath,
        name,
        color,
        hotkey,
        midiNote,
        trimStartMs: trim?.startMs ?? null,
        trimEndMs: trim?.endMs ?? null,
      });
      setStatus(
        t.addPad.importSuccess(pad.name, pad.measuredLufs.toFixed(1), pad.appliedGainDb.toFixed(1), pad.gainCapped),
      );
      onImported();
      await releaseSourceCache();
      onClose();
    } catch (err) {
      setStatus(t.addPad.importError(`${err}`));
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-4">
      <div className="flex w-full max-w-md flex-col gap-4 rounded-xl bg-neutral-900 p-6 shadow-2xl">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t.addPad.title}</h2>
          <button
            className="rounded-full p-1.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
            onClick={handleClose}
            title={t.common.close}
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.common.name}
            <HelpTooltip text={t.common.nameTooltip} />
          </span>
          <input
            className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder={t.common.namePlaceholder}
          />
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.addPad.audioFile}
            <HelpTooltip text={t.addPad.audioFileTooltip} />
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={pickFile}
              className="flex flex-1 items-center gap-2 rounded border border-dashed border-neutral-700 bg-neutral-950 px-3 py-2 text-left hover:border-primary"
            >
              <FileAudio className="h-4 w-4 shrink-0 text-neutral-400" />
              <span className="truncate">{sourcePath ? sourcePath.split("/").pop() : t.addPad.chooseFile}</span>
            </button>
            {sourcePath && (
              <button
                type="button"
                onClick={() => setShowTrimEditor(true)}
                title={t.trimEditor.title}
                className="flex shrink-0 items-center gap-1.5 rounded bg-neutral-800 px-2.5 py-2 text-xs hover:bg-neutral-700"
              >
                <Scissors className="h-3.5 w-3.5" />
                {trim ? `${formatMs(trim.startMs)}–${formatMs(trim.endMs)}` : t.trimEditor.trimButton}
              </button>
            )}
          </div>
        </label>

        <div className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.common.color}
            <HelpTooltip text={t.common.colorTooltip} />
          </span>
          <div className="flex items-center gap-1">
            {DEFAULT_COLORS.map((c) => (
              <button
                key={c}
                aria-label={c}
                onClick={() => setColor(c)}
                className="h-7 w-7 rounded-full border-2"
                style={{ backgroundColor: c, borderColor: c === color ? "white" : "transparent" }}
              />
            ))}
          </div>
        </div>

        <div className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.common.hotkey}
            <HelpTooltip text={t.addPad.hotkeyTooltip} />
          </span>
          <HotkeyPicker value={hotkey} onChange={setHotkey} />
        </div>

        <div className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.common.midiController}
            <HelpTooltip text={t.common.midiControllerTooltip} />
          </span>
          <MidiLearnPicker value={midiNote} onChange={setMidiNote} />
        </div>

        {status && (
          <p className="flex items-center gap-2 text-sm text-neutral-400">
            {importing && <Loader2 className="h-4 w-4 shrink-0 animate-spin" />}
            {status}
          </p>
        )}

        <div className="flex justify-end gap-2 border-t border-neutral-800 pt-4">
          <button className="rounded bg-neutral-800 px-4 py-2 font-medium hover:bg-neutral-700" onClick={handleClose}>
            {t.common.cancel}
          </button>
          <button
            className="flex items-center gap-2 rounded bg-gradient-to-r from-primary to-accent px-4 py-2 font-medium text-white hover:opacity-90 disabled:opacity-40"
            disabled={!sourcePath || !name || importing}
            onClick={handleImport}
          >
            {importing && <Loader2 className="h-4 w-4 shrink-0 animate-spin" />}
            {importing ? t.addPad.importing : t.addPad.submit}
          </button>
        </div>
      </div>

      {showTrimEditor && sourcePath && (
        <TrimEditor sourcePath={sourcePath} onCancel={handleTrimCancel} onConfirm={handleTrimConfirm} />
      )}
    </div>
  );
}

export default AddPadModal;
