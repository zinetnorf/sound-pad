import { Pencil, Play, Repeat, Square } from "lucide-react";
import type { Pad } from "../../types/models";
import { useT } from "../../i18n/LanguageContext";
import ToggleSwitch from "../shared/ToggleSwitch";

interface PadRowProps {
  pad: Pad;
  remainingMs: number | null;
  onTrigger: (padId: string) => void;
  onToggleLoop: (padId: string) => void;
  onEdit: (pad: Pad) => void;
  onDragStart: (padId: string) => void;
  onDragOver: (e: React.DragEvent, padId: string) => void;
  onDrop: () => void;
  isDropTarget: boolean;
}

function PadRow({
  pad,
  remainingMs,
  onTrigger,
  onToggleLoop,
  onEdit,
  onDragStart,
  onDragOver,
  onDrop,
  isDropTarget,
}: PadRowProps) {
  const t = useT();
  const isPlaying = remainingMs !== null;

  return (
    <div
      draggable
      onDragStart={() => onDragStart(pad.id)}
      onDragOver={(e) => onDragOver(e, pad.id)}
      onDrop={onDrop}
      className={`flex cursor-grab items-center gap-4 rounded-lg px-3 py-2 ${
        isDropTarget ? "bg-neutral-800 outline outline-1 outline-primary" : "bg-neutral-900"
      }`}
    >
      <button
        onClick={() => onTrigger(pad.id)}
        title={t.padRow.triggerTitle}
        className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full border-2 transition"
        style={{
          backgroundColor: isPlaying ? pad.color : "transparent",
          borderColor: pad.color,
        }}
      >
        {isPlaying ? (
          <Square className="h-3.5 w-3.5 fill-current text-neutral-950" />
        ) : (
          <Play className="h-4 w-4 fill-current" style={{ color: pad.color }} />
        )}
      </button>

      <span className="w-40 shrink-0 truncate text-sm font-medium">{pad.name}</span>

      <span className="w-16 shrink-0 text-xs tabular-nums text-neutral-500">
        {isPlaying ? `${(remainingMs / 1000).toFixed(1)}s` : ""}
      </span>

      <span className="w-32 shrink-0 text-xs text-neutral-500">
        {pad.hotkey ? t.common.hotkeyBackgroundNote(pad.hotkey) : t.padRow.unassigned}
      </span>

      <div className="flex shrink-0 items-center gap-1.5 text-xs text-neutral-400">
        <ToggleSwitch checked={pad.loopEnabled} onChange={() => onToggleLoop(pad.id)} label={t.padRow.loopShort} />
        <Repeat className="h-3.5 w-3.5" />
        {t.padRow.loopShort}
      </div>

      <button
        className="ml-auto flex shrink-0 items-center gap-1.5 rounded bg-neutral-800 px-3 py-1.5 text-xs font-medium hover:bg-neutral-700"
        onClick={() => onEdit(pad)}
      >
        <Pencil className="h-3.5 w-3.5" />
        {t.common.edit}
      </button>
    </div>
  );
}

export default PadRow;
