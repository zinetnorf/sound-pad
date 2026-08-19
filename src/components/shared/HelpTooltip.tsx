import { CircleHelp } from "lucide-react";

// El atributo nativo `title` no se dispara de forma confiable en el WKWebView
// de Tauri (macOS), así que el tooltip se dibuja a mano con CSS (group-hover).
function HelpTooltip({ text }: { text: string }) {
  return (
    <span className="group relative inline-flex shrink-0 items-center">
      <CircleHelp
        tabIndex={0}
        className="h-3.5 w-3.5 cursor-help text-neutral-500 outline-none transition hover:text-neutral-300 focus-visible:text-neutral-300"
      />
      <span
        role="tooltip"
        className="pointer-events-none absolute bottom-full left-1/2 z-30 mb-2 w-60 -translate-x-1/2 rounded-lg border border-neutral-700 bg-neutral-800 px-3 py-2 text-xs font-normal normal-case leading-snug text-neutral-200 opacity-0 shadow-xl transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100"
      >
        {text}
        <span className="absolute left-1/2 top-full -translate-x-1/2 border-4 border-transparent border-t-neutral-700" />
      </span>
    </span>
  );
}

export default HelpTooltip;
