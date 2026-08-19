import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FilePlus2, OctagonX, Pencil, Play, Settings } from "lucide-react";
import type { AppConfig, Bank } from "./types/models";
import PadList from "./components/config/PadList";
import PadGrid from "./components/live/PadGrid";
import AddPadModal from "./components/config/AddPadModal";
import BankTabs from "./components/shared/BankTabs";
import Logo from "./components/shared/Logo";
import SettingsModal from "./components/settings/SettingsModal";
import { LanguageProvider, useT } from "./i18n/LanguageContext";

type DeviceInfo = {
  id: string;
  label: string;
};

function App() {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [config, setConfig] = useState<AppConfig | null>(null);

  const [midiDevices, setMidiDevices] = useState<string[]>([]);

  const [playing, setPlaying] = useState<Record<string, number>>({});
  const [mode, setMode] = useState<"config" | "live">("live");
  const [hasFocus, setHasFocus] = useState(true);
  const [showSettings, setShowSettings] = useState(false);
  const [showAddPad, setShowAddPad] = useState(false);
  const [deviceWarningLabel, setDeviceWarningLabel] = useState<string | null>(null);

  const activeBank = config?.banks.find((b) => b.id === config.activeBankId);

  useEffect(() => {
    invoke<DeviceInfo[]>("list_output_devices")
      .then(setDevices)
      .catch((err) => console.error("list_output_devices falló:", err));

    loadConfig();

    invoke<string[]>("list_midi_devices")
      .then(setMidiDevices)
      .catch((err) => console.error("list_midi_devices falló:", err));

    const unlistenStarted = listen<{ padId: string; durationMs: number }>("pad-started", (e) => {
      setPlaying((prev) => ({ ...prev, [e.payload.padId]: e.payload.durationMs }));
    });
    const unlistenTick = listen<{ padId: string; remainingMs: number }>("pad-tick", (e) => {
      setPlaying((prev) => ({ ...prev, [e.payload.padId]: e.payload.remainingMs }));
    });
    const unlistenStopped = listen<{ padId: string }>("pad-stopped", (e) => {
      setPlaying((prev) => {
        const next = { ...prev };
        delete next[e.payload.padId];
        return next;
      });
    });
    const unlistenDeviceLost = listen<{ attemptedLabel: string }>("output-device-lost", (e) => {
      setDeviceWarningLabel(e.payload.attemptedLabel);
      loadConfig();
    });

    return () => {
      unlistenStarted.then((f) => f());
      unlistenTick.then((f) => f());
      unlistenStopped.then((f) => f());
      unlistenDeviceLost.then((f) => f());
    };
  }, []);

  // Al perder el foco la ventana, se desactiva el listener de teclas sueltas
  // más abajo — no debe capturar entrada destinada a otras apps (PLAN.md §8).
  useEffect(() => {
    const onFocus = () => setHasFocus(true);
    const onBlur = () => setHasFocus(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  // Tecla suelta en Modo Live con la app en primer plano — la vía rápida sin
  // modificadores (PLAN.md §8). En segundo plano dispara el atajo global ⌃⌥+tecla,
  // manejado enteramente en Rust (hotkeys.rs).
  useEffect(() => {
    if (mode !== "live" || !hasFocus || !activeBank) return;

    function handleKeyDown(e: KeyboardEvent) {
      const target = e.target as HTMLElement;
      if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;
      if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;

      const key = e.key.toUpperCase();
      const pad = activeBank!.pads.find((p) => p.hotkey === key);
      if (pad) {
        e.preventDefault();
        invoke("trigger_pad", { bankId: activeBank!.id, padId: pad.id }).catch((err) =>
          console.error("trigger_pad (tecla suelta) falló:", err),
        );
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, hasFocus, config]);

  async function handlePanic() {
    try {
      await invoke("panic");
    } catch (err) {
      console.error("panic falló:", err);
    }
  }

  async function loadConfig() {
    try {
      const c = await invoke<AppConfig>("get_config");
      setConfig(c);
    } catch (err) {
      console.error("get_config falló:", err);
    }
  }

  return (
    <LanguageProvider language={config?.language ?? "en"}>
      <AppView
        activeBank={activeBank}
        config={config}
        devices={devices}
        midiDevices={midiDevices}
        playing={playing}
        mode={mode}
        setMode={setMode}
        showSettings={showSettings}
        setShowSettings={setShowSettings}
        showAddPad={showAddPad}
        setShowAddPad={setShowAddPad}
        deviceWarningLabel={deviceWarningLabel}
        setDeviceWarningLabel={setDeviceWarningLabel}
        handlePanic={handlePanic}
        loadConfig={loadConfig}
      />
    </LanguageProvider>
  );
}

interface AppViewProps {
  activeBank: Bank | undefined;
  config: AppConfig | null;
  devices: DeviceInfo[];
  midiDevices: string[];
  playing: Record<string, number>;
  mode: "config" | "live";
  setMode: (mode: "config" | "live") => void;
  showSettings: boolean;
  setShowSettings: (v: boolean) => void;
  showAddPad: boolean;
  setShowAddPad: (v: boolean) => void;
  deviceWarningLabel: string | null;
  setDeviceWarningLabel: (v: string | null) => void;
  handlePanic: () => void;
  loadConfig: () => void;
}

function AppView({
  activeBank,
  config,
  devices,
  midiDevices,
  playing,
  mode,
  setMode,
  showSettings,
  setShowSettings,
  showAddPad,
  setShowAddPad,
  deviceWarningLabel,
  setDeviceWarningLabel,
  handlePanic,
  loadConfig,
}: AppViewProps) {
  const t = useT();

  return (
    <main className="flex min-h-screen flex-col gap-6 bg-neutral-950 p-8 text-neutral-100">
      <header className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-10">
          <Logo />
          <button
            onClick={() => setShowAddPad(true)}
            className="flex items-center gap-2 rounded-full bg-gradient-to-r from-primary to-accent px-4 py-2.5 text-sm font-semibold text-white shadow-lg shadow-primary/20 hover:opacity-90"
          >
            <FilePlus2 className="h-4 w-4" />
            {t.app.addPad}
          </button>
        </div>

        <div className="flex gap-2">
          <button
            onClick={() => setShowSettings(true)}
            className="flex items-center gap-2 rounded-full bg-neutral-800 px-4 py-2.5 text-sm font-medium text-neutral-200 shadow-lg hover:bg-neutral-700"
            title={t.app.settingsTitle}
          >
            <Settings className="h-4 w-4" />
            {t.app.config}
          </button>
          <button
            onClick={handlePanic}
            className="flex items-center gap-2 rounded-full bg-red-700 px-5 py-2.5 text-sm font-bold text-white shadow-lg hover:bg-red-600"
            title={t.app.panicTitle}
          >
            <OctagonX className="h-4 w-4" />
            {t.app.panic}
          </button>
        </div>
      </header>

      {deviceWarningLabel && (
        <div className="rounded border border-yellow-700 bg-yellow-950 px-4 py-3 text-sm text-yellow-200">
          ⚠ {t.app.deviceLost(deviceWarningLabel)}
          <button className="ml-3 underline hover:no-underline" onClick={() => setDeviceWarningLabel(null)}>
            {t.common.close}
          </button>
        </div>
      )}

      {showSettings && config && (
        <SettingsModal
          config={config}
          outputDevices={devices}
          midiDevices={midiDevices}
          onClose={() => setShowSettings(false)}
          onChanged={loadConfig}
        />
      )}

      {showAddPad && <AddPadModal onImported={loadConfig} onClose={() => setShowAddPad(false)} />}

      <section className="flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <BankTabs banks={config?.banks ?? []} activeBankId={config?.activeBankId ?? ""} onChanged={loadConfig} />
          <div className="flex overflow-hidden rounded-full border border-neutral-700 text-sm">
            <button
              onClick={() => setMode("live")}
              className={`flex items-center gap-1.5 px-3 py-1.5 font-medium transition ${
                mode === "live"
                  ? "bg-gradient-to-r from-primary to-accent text-white"
                  : "bg-neutral-900 text-neutral-400 hover:bg-neutral-800"
              }`}
            >
              <Play className="h-3.5 w-3.5" />
              {t.app.live}
            </button>
            <button
              onClick={() => setMode("config")}
              className={`flex items-center gap-1.5 px-3 py-1.5 font-medium transition ${
                mode === "config"
                  ? "bg-gradient-to-r from-primary to-accent text-white"
                  : "bg-neutral-900 text-neutral-400 hover:bg-neutral-800"
              }`}
            >
              <Pencil className="h-3.5 w-3.5" />
              {t.app.editMode}
            </button>
          </div>
        </div>
        {activeBank ? (
          mode === "config" ? (
            <PadList bank={activeBank} playing={playing} onChanged={loadConfig} />
          ) : (
            <PadGrid bank={activeBank} playing={playing} />
          )
        ) : (
          <p className="text-sm text-neutral-500">{t.app.noBanksHint}</p>
        )}
      </section>
    </main>
  );
}

export default App;
