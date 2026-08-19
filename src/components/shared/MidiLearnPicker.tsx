import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Music2 } from "lucide-react";
import { useT } from "../../i18n/LanguageContext";

interface MidiLearnPickerProps {
  value: number | null;
  onChange: (note: number | null) => void;
}

function MidiLearnPicker({ value, onChange }: MidiLearnPickerProps) {
  const t = useT();
  const [listening, setListening] = useState(false);

  useEffect(() => {
    if (!listening) return;

    const unlisten = listen<{ note: number }>("midi-learn-captured", (e) => {
      onChange(e.payload.note);
      setListening(false);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, [listening, onChange]);

  async function startLearn() {
    try {
      await invoke("start_midi_learn");
      setListening(true);
    } catch (err) {
      console.error("start_midi_learn falló:", err);
    }
  }

  async function cancel() {
    try {
      await invoke("cancel_midi_learn");
    } catch (err) {
      console.error("cancel_midi_learn falló:", err);
    }
    setListening(false);
  }

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={listening ? cancel : startLearn}
        className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700"
      >
        <Music2 className="h-3.5 w-3.5" />
        {listening ? t.midiPicker.listening : value !== null ? t.midiPicker.assigned(value) : t.midiPicker.assign}
      </button>
      {value !== null && !listening && (
        <button
          type="button"
          onClick={() => onChange(null)}
          className="text-xs text-neutral-500 hover:text-neutral-300"
        >
          {t.common.remove}
        </button>
      )}
    </div>
  );
}

export default MidiLearnPicker;
