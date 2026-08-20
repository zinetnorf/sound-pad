import { invoke } from "@tauri-apps/api/core";
import type { Bank } from "../../types/models";
import PadTile from "./PadTile";
import { useT } from "../../i18n/LanguageContext";

interface PadGridProps {
  bank: Bank;
  playing: Record<string, number>;
  columns: number;
}

function PadGrid({ bank, playing, columns }: PadGridProps) {
  const t = useT();
  async function handleTrigger(padId: string) {
    try {
      await invoke("trigger_pad", { bankId: bank.id, padId });
    } catch (err) {
      console.error("trigger_pad falló:", err);
    }
  }

  // Defensivo: una config editada a mano podría traer un valor fuera de rango
  // (o 0, que rompería el grid entero).
  const safeColumns = Math.min(6, Math.max(3, columns || 4));

  return (
    <div className="grid gap-4" style={{ gridTemplateColumns: `repeat(${safeColumns}, minmax(0, 1fr))` }}>
      {bank.pads.map((pad) => (
        <PadTile
          key={pad.id}
          pad={pad}
          remainingMs={pad.id in playing ? playing[pad.id] : null}
          onTrigger={handleTrigger}
        />
      ))}
      {bank.pads.length === 0 && <p className="text-sm text-neutral-500">{t.padGrid.empty}</p>}
    </div>
  );
}

export default PadGrid;
