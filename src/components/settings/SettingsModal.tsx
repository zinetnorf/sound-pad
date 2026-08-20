import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, Upload, Volume2, X } from "lucide-react";
import type { AppConfig, AppLanguage } from "../../types/models";
import HelpTooltip from "../shared/HelpTooltip";
import HotkeyPicker from "../shared/HotkeyPicker";
import { useT } from "../../i18n/LanguageContext";

interface SettingsModalProps {
  config: AppConfig;
  outputDevices: { id: string; label: string }[];
  midiDevices: string[];
  onClose: () => void;
  onChanged: () => void;
}

function SettingsModal({ config, outputDevices, midiDevices, onClose, onChanged }: SettingsModalProps) {
  const t = useT();
  const [targetLufs, setTargetLufs] = useState(config.targetLufs);
  const [renormStatus, setRenormStatus] = useState("");
  const [testStatus, setTestStatus] = useState("");
  const [error, setError] = useState("");

  async function handleRenormalize() {
    setRenormStatus(t.settings.renormalizing);
    try {
      await invoke("renormalize_all", { targetLufs });
      setRenormStatus(t.common.done);
      onChanged();
    } catch (err) {
      setRenormStatus(t.common.errorPrefix(`${err}`));
    }
  }

  async function updateConfig(patch: Partial<AppConfig>) {
    try {
      await invoke("save_config", { config: { ...config, ...patch } });
      onChanged();
    } catch (err) {
      setError(`${err}`);
    }
  }

  async function handleSetOutputDevice(deviceId: string) {
    try {
      await invoke("set_output_device", { deviceId });
      onChanged();
    } catch (err) {
      setError(`${err}`);
    }
  }

  async function handleTestOutputDevice() {
    setTestStatus(t.settings.playing);
    try {
      await invoke("play_test_tone", { deviceId: config.outputDevice.deviceId });
      setTestStatus(t.common.done);
    } catch (err) {
      setTestStatus(t.common.errorPrefix(`${err}`));
    }
  }

  async function handleSetMidiDevice(deviceName: string) {
    try {
      await invoke("set_midi_device", { deviceName: deviceName || null });
      onChanged();
    } catch (err) {
      setError(`${err}`);
    }
  }

  async function handleExport() {
    const dest = await save({ defaultPath: "soundpad-export.zip", filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (!dest) return;
    try {
      await invoke("export_config", { destPath: dest });
      setError("");
    } catch (err) {
      setError(t.settings.exportError(`${err}`));
    }
  }

  async function handleImport() {
    const src = await open({ multiple: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
    if (typeof src !== "string") return;
    try {
      await invoke("import_config", { sourcePath: src });
      onChanged();
    } catch (err) {
      setError(t.settings.importError(`${err}`));
    }
  }

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-4">
      <div className="flex max-h-[90vh] w-full max-w-md flex-col gap-4 overflow-y-auto rounded-xl bg-neutral-900 p-6 shadow-2xl">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t.settings.title}</h2>
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
            {t.settings.outputDevice}
            <HelpTooltip text={t.settings.outputDeviceTooltip} />
          </span>
          <div className="flex items-center gap-2">
            <select
              className="flex-1 rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
              value={config.outputDevice.deviceId}
              onChange={(e) => handleSetOutputDevice(e.currentTarget.value)}
            >
              {outputDevices.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.label}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={handleTestOutputDevice}
              className="flex shrink-0 items-center gap-1.5 rounded bg-neutral-800 px-3 py-2 text-sm font-medium hover:bg-neutral-700"
              title={t.settings.testDeviceTitle}
            >
              <Volume2 className="h-4 w-4" />
              {t.settings.test}
            </button>
          </div>
          {testStatus && <span className="text-xs text-neutral-500">{testStatus}</span>}
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.settings.midiDevice}
            <HelpTooltip text={t.settings.midiDeviceTooltip} />
          </span>
          <select
            className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
            value={config.midiDeviceName ?? ""}
            onChange={(e) => handleSetMidiDevice(e.currentTarget.value)}
          >
            <option value="">{t.settings.noMidiDevice}</option>
            {midiDevices.map((d) => (
              <option key={d} value={d}>
                {d}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            Idioma / Language
            <HelpTooltip text={t.settings.languageTooltip} />
          </span>
          <select
            className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
            value={config.language}
            onChange={(e) => updateConfig({ language: e.currentTarget.value as AppLanguage })}
          >
            <option value="en">English</option>
            <option value="es">Español</option>
          </select>
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.settings.gridColumns}
            <HelpTooltip text={t.settings.gridColumnsTooltip} />
          </span>
          <select
            className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
            value={config.gridColumns}
            onChange={(e) => updateConfig({ gridColumns: Number(e.currentTarget.value) })}
          >
            {[3, 4, 5, 6].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>

        <div className="flex items-end gap-2">
          <label className="flex flex-1 flex-col gap-1 text-sm">
            <span className="flex items-center gap-1">
              {t.settings.targetLufs}
              <HelpTooltip text={t.settings.targetLufsTooltip} />
            </span>
            <input
              type="number"
              step={0.5}
              className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
              value={targetLufs}
              onChange={(e) => setTargetLufs(parseFloat(e.currentTarget.value))}
            />
          </label>
          <button className="rounded bg-neutral-800 px-3 py-2 text-sm hover:bg-neutral-700" onClick={handleRenormalize}>
            {t.settings.renormalizeAll}
          </button>
        </div>
        {renormStatus && <p className="text-xs text-neutral-500">{renormStatus}</p>}

        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={config.globalHotkeysEnabled}
            onChange={(e) => updateConfig({ globalHotkeysEnabled: e.currentTarget.checked })}
            className="accent-primary"
          />
          <span className="flex items-center gap-1">
            {t.settings.globalHotkeys}
            <HelpTooltip text={t.settings.globalHotkeysTooltip} />
          </span>
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.settings.globalModifier}
            <HelpTooltip text={t.settings.globalModifierTooltip} />
          </span>
          <input
            className="rounded border border-neutral-700 bg-neutral-950 px-3 py-2 focus:border-primary focus:outline-none"
            value={config.globalModifier}
            onChange={(e) => updateConfig({ globalModifier: e.currentTarget.value })}
          />
        </label>

        <div className="flex flex-col gap-1 text-sm">
          <span className="flex items-center gap-1">
            {t.settings.panicHotkey}
            <HelpTooltip text={t.settings.panicHotkeyTooltip} />
          </span>
          <HotkeyPicker value={config.panicHotkey} onChange={(panicHotkey) => updateConfig({ panicHotkey })} />
        </div>

        <div className="flex gap-2 border-t border-neutral-800 pt-4">
          <button
            className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-2 text-sm hover:bg-neutral-700"
            onClick={handleExport}
          >
            <Download className="h-4 w-4" />
            {t.settings.export}
          </button>
          <button
            className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-2 text-sm hover:bg-neutral-700"
            onClick={handleImport}
          >
            <Upload className="h-4 w-4" />
            {t.settings.import}
          </button>
        </div>

        {error && <p className="text-sm text-red-400">{error}</p>}

        <button
          className="self-end rounded bg-gradient-to-r from-primary to-accent px-4 py-2 font-medium text-white hover:opacity-90"
          onClick={onClose}
        >
          {t.common.close}
        </button>
      </div>
    </div>
  );
}

export default SettingsModal;
