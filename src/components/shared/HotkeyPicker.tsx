import { useEffect, useState } from "react";
import { Keyboard } from "lucide-react";
import { useT } from "../../i18n/LanguageContext";

interface HotkeyPickerProps {
  value: string | null;
  onChange: (key: string | null) => void;
}

function HotkeyPicker({ value, onChange }: HotkeyPickerProps) {
  const t = useT();
  const [listening, setListening] = useState(false);

  useEffect(() => {
    if (!listening) return;

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        setListening(false);
        return;
      }
      const key = e.key.toUpperCase();
      if (/^[A-Z0-9]$/.test(key)) {
        e.preventDefault();
        onChange(key);
        setListening(false);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [listening, onChange]);

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={() => setListening(true)}
        className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700"
      >
        <Keyboard className="h-3.5 w-3.5" />
        {listening ? t.hotkeyPicker.listening : value ? t.hotkeyPicker.assigned(value) : t.hotkeyPicker.assign}
      </button>
      {value && !listening && (
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

export default HotkeyPicker;
