import { useState } from "react";
import { Trash2, X } from "lucide-react";
import type { Pad, RetriggerMode } from "../../types/models";
import HelpTooltip from "../shared/HelpTooltip";
import HotkeyPicker from "../shared/HotkeyPicker";
import MidiLearnPicker from "../shared/MidiLearnPicker";
import ToggleSwitch from "../shared/ToggleSwitch";
import { useT } from "../../i18n/LanguageContext";

const DEFAULT_COLORS = ["#E8734A", "#4AA3E8", "#7CE84A", "#E8D24A", "#B14AE8"];
const RETRIGGER_MODES: RetriggerMode[] = ["Stop", "Restart", "Ignore", "Overlap"];

interface PadEditModalProps {
  pad: Pad;
  error?: string;
  onSave: (pad: Pad) => void;
  onDelete: (pad: Pad) => void;
  onClose: () => void;
}

function PadEditModal({ pad, error, onSave, onDelete, onClose }: PadEditModalProps) {
  const t = useT();
  const [draft, setDraft] = useState<Pad>(pad);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-4">
      <div className="flex w-full max-w-md flex-col gap-4 rounded-xl bg-neutral-900 p-6 shadow-2xl">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t.editPad.title}</h2>
          <button
            className="rounded-full p-1.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
            onClick={onClose}
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
            className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2"
            value={draft.name}
            onChange={(e) => setDraft({ ...draft, name: e.currentTarget.value })}
          />
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
                onClick={() => setDraft({ ...draft, color: c })}
                className="h-7 w-7 rounded-full border-2"
                style={{ backgroundColor: c, borderColor: c === draft.color ? "white" : "transparent" }}
              />
            ))}
          </div>
        </div>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.editPad.volume(draft.trimDb.toFixed(1))}
            <HelpTooltip text={t.editPad.volumeTooltip} />
          </span>
          <input
            type="range"
            min={-12}
            max={12}
            step={0.5}
            value={draft.trimDb}
            onChange={(e) => setDraft({ ...draft, trimDb: parseFloat(e.currentTarget.value) })}
          />
        </label>

        <div className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.common.hotkey}
            <HelpTooltip text={t.common.hotkeyTooltip} />
          </span>
          <HotkeyPicker value={draft.hotkey} onChange={(hotkey) => setDraft({ ...draft, hotkey })} />
          {draft.hotkey && (
            <span className="text-xs text-neutral-500">{t.common.hotkeyBackgroundNote(draft.hotkey)}</span>
          )}
        </div>

        <div className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.common.midiController}
            <HelpTooltip text={t.common.midiControllerTooltip} />
          </span>
          <MidiLearnPicker value={draft.midiNote} onChange={(midiNote) => setDraft({ ...draft, midiNote })} />
        </div>

        <div className="flex items-center gap-2 text-sm">
          <ToggleSwitch
            checked={draft.loopEnabled}
            onChange={(loopEnabled) => setDraft({ ...draft, loopEnabled })}
            label={t.editPad.loop}
          />
          <span className="flex items-center gap-1">
            {t.editPad.loop}
            <HelpTooltip text={t.editPad.loopTooltip} />
          </span>
        </div>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.editPad.retriggerMode}
            <HelpTooltip text={t.editPad.retriggerModeTooltip} />
          </span>
          <select
            className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2"
            value={draft.retriggerMode}
            onChange={(e) => setDraft({ ...draft, retriggerMode: e.currentTarget.value as RetriggerMode })}
          >
            {RETRIGGER_MODES.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </label>

        <div className="flex gap-3">
          <label className="flex flex-1 flex-col gap-1 text-sm">
            <span className="flex items-center gap-1">
              {t.editPad.fadeIn}
              <HelpTooltip text={t.editPad.fadeTooltip} />
            </span>
            <input
              type="number"
              min={0}
              className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2"
              value={draft.fadeInMs}
              onChange={(e) => setDraft({ ...draft, fadeInMs: parseInt(e.currentTarget.value) || 0 })}
            />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-sm">
            {t.editPad.fadeOut}
            <input
              type="number"
              min={0}
              className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2"
              value={draft.fadeOutMs}
              onChange={(e) => setDraft({ ...draft, fadeOutMs: parseInt(e.currentTarget.value) || 0 })}
            />
          </label>
        </div>

        {error && <p className="text-sm text-red-400">{error}</p>}

        <div className="flex items-center justify-between border-t border-neutral-800 pt-4">
          {confirmingDelete ? (
            <div className="flex items-center gap-2 text-sm">
              <span className="text-neutral-400">{t.editPad.confirmDelete}</span>
              <button
                className="rounded bg-red-700 px-3 py-1.5 font-medium hover:bg-red-600"
                onClick={() => onDelete(draft)}
              >
                {t.editPad.confirmDeleteYes}
              </button>
              <button
                className="rounded bg-neutral-800 px-3 py-1.5 hover:bg-neutral-700"
                onClick={() => setConfirmingDelete(false)}
              >
                {t.common.cancel}
              </button>
            </div>
          ) : (
            <button
              className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-1.5 text-sm font-medium text-red-400 hover:bg-neutral-700"
              onClick={() => setConfirmingDelete(true)}
            >
              <Trash2 className="h-3.5 w-3.5" />
              {t.editPad.delete}
            </button>
          )}

          <div className="flex gap-2">
            <button className="rounded bg-neutral-800 px-4 py-2 font-medium hover:bg-neutral-700" onClick={onClose}>
              {t.common.cancel}
            </button>
            <button
              className="rounded bg-gradient-to-r from-primary to-accent px-4 py-2 font-medium text-white hover:opacity-90"
              onClick={() => onSave(draft)}
            >
              {t.common.save}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default PadEditModal;
