// Espejo de src-tauri/src/models.rs — mantener sincronizado a mano (PLAN.md §5).

export type RetriggerMode = "Stop" | "Restart" | "Ignore" | "Overlap";

export type AppLanguage = "en" | "es";

export interface OutputDeviceRef {
  deviceId: string;
  label: string;
}

export interface Pad {
  id: string;
  name: string;
  order: number;
  color: string;
  trimDb: number;
  loopEnabled: boolean;
  retriggerMode: RetriggerMode;
  fadeInMs: number;
  fadeOutMs: number;
  hotkey: string | null;
  midiNote: number | null;
  durationMs: number;

  originalFilename: string;
  sourceSampleRate: number;
  sourceChannels: number;
  measuredLufs: number;
  appliedGainDb: number;
  gainCapped: boolean;
}

export interface Bank {
  id: string;
  name: string;
  order: number;
  pads: Pad[];
}

export interface AppConfig {
  version: number;
  banks: Bank[];
  activeBankId: string;
  outputDevice: OutputDeviceRef;
  outputBufferFrames: number;
  midiDeviceName: string | null;
  targetLufs: number;
  globalHotkeysEnabled: boolean;
  globalModifier: string;
  panicHotkey: string | null;
  language: AppLanguage;
}
