import React, { useState, useEffect } from "react";
import { Languages, History, Package, Settings } from "lucide-react";
import type { AppView, TranslationHistoryEntry } from "./types";
import type { Locale } from "./i18n";
import { I18nProvider, useI18n } from "./i18n-context";
import TranslatorPanel from "./components/TranslatorPanel";
import ModelManager from "./components/ModelManager";
import HistoryPanel from "./components/HistoryPanel";
import SettingsPanel from "./components/SettingsPanel";
import FloatingButton from "./components/FloatingButton";
import { getModelStatus, getSettings } from "./api";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import "./styles/App.css";

// ── Floating button window (separate window rendered at /?window=floating)
const isFloatingWindow = new URLSearchParams(window.location.search).get("window") === "floating";

export default function App() {
  if (isFloatingWindow) {
    return <FloatingButton />;
  }

  return (
    <I18nProvider initial="en">
      <MainAppWithLocale />
    </I18nProvider>
  );
}

function MainAppWithLocale() {
  const { setLocale } = useI18n();
  const [localeReady, setLocaleReady] = useState(false);

  useEffect(() => {
    getSettings().then((s) => {
      if (s.locale === "ru" || s.locale === "en") {
        setLocale(s.locale as Locale);
      }
      setLocaleReady(true);
    }).catch(() => setLocaleReady(true));
  }, []);

  if (!localeReady) return null;
  return <MainApp />;
}

function MainApp() {
  const { t, locale, setLocale } = useI18n();
  const [view, setView] = useState<AppView>("onboarding");
  const [modelReady, setModelReady] = useState(false);
  const [historyEntry, setHistoryEntry] = useState<TranslationHistoryEntry | null>(null);
  const [glossary, setGlossary] = useState<{ source: string; target: string; lang_pair: string }[]>([]);
  const [injectedText, setInjectedText] = useState<string | undefined>(undefined);
  const [translateReplaceActive, setTranslateReplaceActive] = useState(false);
  const [defaultTargetLang, setDefaultTargetLang] = useState("en");
  const [defaultSourceLang, setDefaultSourceLang] = useState("auto");

  useEffect(() => {
    getModelStatus().then((status) => {
      if (status.type === "ready") {
        setModelReady(true);
        setView("translator");
      }
    }).catch(() => {});

    getSettings().then((s) => {
      setGlossary(s.glossary);
      setDefaultTargetLang(s.default_target_lang);
      setDefaultSourceLang(s.default_source_lang);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    const subs: Promise<() => void>[] = [];

    subs.push(listen("model_ready", () => {
      setModelReady(true);
      if (view === "onboarding" || view === "model_manager") setView("translator");
    }));

    subs.push(listen<{ text: string }>("insert_text", (e) => {
      setView("translator");
      setInjectedText(e.payload.text);
    }));

    subs.push(listen("translate_replace_started", () => {
      setTranslateReplaceActive(true);
    }));
    subs.push(listen("translate_replace_done", () => {
      setTranslateReplaceActive(false);
    }));

    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, [view]);

  useEffect(() => {
    const sub = listen("hotkey_translate_replace", () => {
      if (!modelReady) return;
      invoke("translate_and_replace", {
        sourceLang: defaultSourceLang,
        targetLang: defaultTargetLang,
      }).catch(console.error);
    });
    return () => { sub.then((f) => f()); };
  }, [modelReady, defaultSourceLang, defaultTargetLang]);

  const handleModelReady = () => {
    setModelReady(true);
    setView("translator");
  };

  const handleHistorySelect = (entry: TranslationHistoryEntry) => {
    setHistoryEntry(entry);
    setView("translator");
  };

  const handleInjectedConsumed = () => setInjectedText(undefined);

  return (
    <div className="app">
      <nav className="sidebar">
        <div className="sidebar-logo">DeepM</div>
        <div className="sidebar-nav">
          <NavBtn active={view === "translator"} onClick={() => setView("translator")} icon={<Languages size={20} />} label={t.nav_translate} />
          <NavBtn active={view === "history"} onClick={() => setView("history")} icon={<History size={20} />} label={t.nav_history} />
          <NavBtn active={view === "model_manager"} onClick={() => setView("model_manager")} icon={<Package size={20} />} label={t.nav_model} />
          <NavBtn active={view === "settings"} onClick={() => setView("settings")} icon={<Settings size={20} />} label={t.nav_settings} />
        </div>
        <div className="sidebar-bottom">
          <div className={`model-status-dot ${modelReady ? "ready" : "not-ready"}`} />
          <span className="model-status-label">{modelReady ? t.model_ready : t.no_model}</span>
        </div>
      </nav>

      <main className="main-content">
        {translateReplaceActive && (
          <div className="translate-replace-banner">
            <span className="tr-spinner" /> {t.translating_in_place}
          </div>
        )}

        {view === "onboarding" && <ModelManager onModelReady={handleModelReady} isOnboarding />}
        {view === "translator" && (
          <TranslatorPanel
            glossaryEntries={glossary}
            initialText={injectedText ?? historyEntry?.source_text}
            onInitialTextConsumed={handleInjectedConsumed}
            defaultSourceLang={defaultSourceLang}
            defaultTargetLang={defaultTargetLang}
          />
        )}
        {view === "history" && <HistoryPanel onSelect={handleHistorySelect} />}
        {view === "model_manager" && <ModelManager onModelReady={handleModelReady} />}
        {view === "settings" && (
          <SettingsPanel
            onClose={() => setView("translator")}
            locale={locale}
            onLocaleChange={(l) => setLocale(l)}
          />
        )}
      </main>
    </div>
  );
}

function NavBtn({ active, onClick, icon, label }: {
  active: boolean; onClick: () => void; icon: React.ReactNode; label: string;
}) {
  return (
    <button className={`nav-btn ${active ? "active" : ""}`} onClick={onClick} title={label}>
      <span className="nav-icon">{icon}</span>
      <span className="nav-label">{label}</span>
    </button>
  );
}
