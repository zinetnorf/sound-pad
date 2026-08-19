import { invoke } from "@tauri-apps/api/core";
import type { Bank } from "../../types/models";
import PadTile from "./PadTile";
import { useT } from "../../i18n/LanguageContext";

interface PadGridProps {
  bank: Bank;
  playing: Record<string, number>;
}

function PadGrid({ bank, playing }: PadGridProps) {
  const t = useT();
  async function handleTrigger(padId: string) {
    try {
      await invoke("trigger_pad", { bankId: bank.id, padId });
    } catch (err) {
      console.error("trigger_pad falló:", err);
    }
  }

  return (
    <div className="grid gap-4" style={{ gridTemplateColumns: "repeat(4, minmax(0, 1fr))" }}>
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
