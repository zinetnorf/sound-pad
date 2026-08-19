import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search } from "lucide-react";
import type { Bank, Pad } from "../../types/models";
import PadRow from "./PadRow";
import PadEditModal from "./PadEditModal";
import HelpTooltip from "../shared/HelpTooltip";
import { useT } from "../../i18n/LanguageContext";

interface PadListProps {
  bank: Bank;
  playing: Record<string, number>;
  onChanged: () => void;
}

function PadList({ bank, playing, onChanged }: PadListProps) {
  const t = useT();
  const [query, setQuery] = useState("");
  const [editingPad, setEditingPad] = useState<Pad | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [editError, setEditError] = useState("");

  const filtered = bank.pads.filter((p) => p.name.toLowerCase().includes(query.toLowerCase()));

  async function handleTrigger(padId: string) {
    try {
      await invoke("trigger_pad", { bankId: bank.id, padId });
    } catch (err) {
      setError(t.common.triggerError(`${err}`));
    }
  }

  /** Lanza en error — quien llama decide si eso debe cerrar algo o no. */
  async function persist(pad: Pad) {
    await invoke("update_pad", { bankId: bank.id, pad });
    onChanged();
  }

  function handleToggleLoop(padId: string) {
    const pad = bank.pads.find((p) => p.id === padId);
    if (pad) persist({ ...pad, loopEnabled: !pad.loopEnabled }).catch((err) => setError(t.common.saveError(`${err}`)));
  }

  async function handleSaveEdit(pad: Pad) {
    try {
      await persist(pad);
      setEditError("");
      setEditingPad(null);
    } catch (err) {
      // No cerramos el modal: un choque de tecla duplicada (PLAN.md §8) hay
      // que poder corregirlo sin perder el resto de los cambios del draft.
      setEditError(`${err}`);
    }
  }

  async function handleDelete(pad: Pad) {
    try {
      await invoke("delete_pad", { bankId: bank.id, padId: pad.id });
      setEditingPad(null);
      onChanged();
    } catch (err) {
      setError(t.common.deleteError(`${err}`));
    }
  }

  function handleDragStart(padId: string) {
    setDragId(padId);
  }

  function handleDragOver(e: React.DragEvent, padId: string) {
    e.preventDefault();
    if (padId !== dragId) setDropTargetId(padId);
  }

  async function handleDrop() {
    if (!dragId || !dropTargetId || dragId === dropTargetId) {
      setDragId(null);
      setDropTargetId(null);
      return;
    }
    const ids = bank.pads.map((p) => p.id);
    const fromIdx = ids.indexOf(dragId);
    const toIdx = ids.indexOf(dropTargetId);
    ids.splice(toIdx, 0, ids.splice(fromIdx, 1)[0]);

    setDragId(null);
    setDropTargetId(null);
    try {
      await invoke("reorder_pads", { bankId: bank.id, orderedPadIds: ids });
      onChanged();
    } catch (err) {
      setError(t.padList.reorderError(`${err}`));
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-4 text-xs text-neutral-500">
        <div className="relative w-64">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-500" />
          <input
            className="w-full rounded border border-neutral-700 bg-neutral-900 py-1.5 pl-8 pr-3 text-sm text-neutral-200 focus:border-primary focus:outline-none"
            placeholder={t.padList.searchPlaceholder}
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
          />
        </div>
        <span className="ml-24 flex items-center gap-1">
          {t.common.hotkey}
          <HelpTooltip text={t.common.hotkeyTooltip} />
        </span>
      </div>

      <div className="flex flex-col gap-1" onDragEnd={() => { setDragId(null); setDropTargetId(null); }}>
        {filtered.map((pad) => (
          <PadRow
            key={pad.id}
            pad={pad}
            remainingMs={pad.id in playing ? playing[pad.id] : null}
            onTrigger={handleTrigger}
            onToggleLoop={handleToggleLoop}
            onEdit={setEditingPad}
            onDragStart={handleDragStart}
            onDragOver={handleDragOver}
            onDrop={handleDrop}
            isDropTarget={dropTargetId === pad.id}
          />
        ))}
        {filtered.length === 0 && <p className="text-sm text-neutral-500">{t.padList.noResults}</p>}
      </div>

      {error && <p className="text-sm text-red-400">{error}</p>}

      {editingPad && (
        <PadEditModal
          pad={editingPad}
          error={editError}
          onSave={handleSaveEdit}
          onDelete={handleDelete}
          onClose={() => {
            setEditError("");
            setEditingPad(null);
          }}
        />
      )}
    </div>
  );
}

export default PadList;
