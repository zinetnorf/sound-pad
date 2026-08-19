import type { Pad } from "../../types/models";
import { shadeColor } from "../../lib/color";

interface PadTileProps {
  pad: Pad;
  remainingMs: number | null;
  onTrigger: (padId: string) => void;
}

function PadTile({ pad, remainingMs, onTrigger }: PadTileProps) {
  const isPlaying = remainingMs !== null;

  const background = isPlaying
    ? `linear-gradient(135deg, ${shadeColor(pad.color, 18)}, ${shadeColor(pad.color, -28)})`
    : `linear-gradient(135deg, ${pad.color}40, ${pad.color}14)`;

  return (
    <button
      onClick={() => onTrigger(pad.id)}
      className="flex aspect-square flex-col items-center justify-center gap-2 rounded-xl p-3 text-center transition"
      style={{
        background,
        boxShadow: isPlaying ? `0 0 0 3px ${pad.color}, 0 0 20px 4px ${pad.color}66` : "none",
      }}
    >
      <span
        className="line-clamp-2 text-sm font-semibold leading-tight"
        style={{ color: isPlaying ? "#0a0a0a" : "#e5e5e5" }}
      >
        {pad.name}
      </span>
      {isPlaying && (
        <span className="text-sm font-bold tabular-nums" style={{ color: "#0a0a0a" }}>
          {(remainingMs / 1000).toFixed(1)}s
        </span>
      )}
    </button>
  );
}

export default PadTile;
