import { createContext, useContext } from "react";
import type { AppLanguage } from "../types/models";
import { type Dictionary, en, es } from "./dictionary";

const dictionaries: Record<AppLanguage, Dictionary> = { en, es };

const LanguageContext = createContext<Dictionary>(en);

function LanguageProvider({ language, children }: { language: AppLanguage; children: React.ReactNode }) {
  return <LanguageContext.Provider value={dictionaries[language]}>{children}</LanguageContext.Provider>;
}

function useT(): Dictionary {
  return useContext(LanguageContext);
}

export { LanguageProvider, useT };
