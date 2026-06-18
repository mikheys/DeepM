import React, { createContext, useContext, useState } from "react";
import { type Locale, type Strings, getStrings } from "./i18n";

type I18nCtx = {
  locale: Locale;
  t: Strings;
  setLocale: (l: Locale) => void;
};

const I18nContext = createContext<I18nCtx>({
  locale: "en",
  t: getStrings("en"),
  setLocale: () => {},
});

export function I18nProvider({ children, initial = "en" }: { children: React.ReactNode; initial?: Locale }) {
  const [locale, setLocaleState] = useState<Locale>(initial);

  const setLocale = (l: Locale) => {
    setLocaleState(l);
  };

  return (
    <I18nContext.Provider value={{ locale, t: getStrings(locale), setLocale }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n() {
  return useContext(I18nContext);
}
