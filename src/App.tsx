import React, { useState, useEffect, useCallback } from "react";
import {
  Languages, History, Package, Settings,
  ChevronRight, ChevronLeft,
} from "lucide-react";
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
  const [view, setView] = useState<AppView>("translator");
  const [modelReady, setModelReady] = useState(false);
  const [historyEntry, setHistoryEntry] = useState<TranslationHistoryEntry | null>(null);
  const [glossary, setGlossary] = useState<{ source: string; target: string; lang_pair: string }[]>([]);
  const [injectedText, setInjectedText] = useState<string | undefined>(undefined);
  const [translateReplaceActive, setTranslateReplaceActive] = useState(false);
  const [defaultTargetLang, setDefaultTargetLang] = useState("en");
  const [defaultSourceLang, setDefaultSourceLang] = useState("auto");

  // Sidebar: compact by default, persisted in localStorage
  const [sidebarExpanded, setSidebarExpanded] = useState(
    () => localStorage.getItem("sidebarExpanded") === "true"
  );
  const toggleSidebar = useCallback(() => {
    setSidebarExpanded((prev) => {
      const next = !prev;
      localStorage.setItem("sidebarExpanded", String(next));
      return next;
    });
  }, []);

  // Model status: poll for first 60s as fallback if model_ready event is missed
  useEffect(() => {
    let cancelled = false;
    let attempts = 0;

    const check = () => {
      if (cancelled) return;
      getModelStatus()
        .then((s) => {
          if (s.type === "ready") {
            setModelReady(true);
          } else if (attempts < 12) {
            attempts++;
            setTimeout(check, 5000);
          }
        })
        .catch(() => {
          if (attempts < 12) {
            attempts++;
            setTimeout(check, 5000);
          }
        });
    };

    check();
    return () => { cancelled = true; };
  }, []);

  // model_ready listener — stable (no [view] dependency so it's never torn down)
  useEffect(() => {
    const unsub = listen("model_ready", () => setModelReady(true));
    return () => { unsub.then((f) => f()); };
  }, []);

  // Settings load
  useEffect(() => {
    getSettings().then((s) => {
      setGlossary(s.glossary);
      setDefaultTargetLang(s.default_target_lang);
      setDefaultSourceLang(s.default_source_lang);
    }).catch(() => {});
  }, []);

  // Other event listeners
  useEffect(() => {
    const subs: Promise<() => void>[] = [];

    subs.push(listen<{ text: string }>("insert_text", (e) => {
      setView("translator");
      setInjectedText(e.payload.text);
    }));
    subs.push(listen("translate_replace_started", () => setTranslateReplaceActive(true)));
    subs.push(listen("translate_replace_done", () => setTranslateReplaceActive(false)));

    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, []);

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

  const handleModelReady = () => { setModelReady(true); setView("translator"); };
  const handleHistorySelect = (entry: TranslationHistoryEntry) => {
    setHistoryEntry(entry);
    setView("translator");
  };
  const handleInjectedConsumed = () => setInjectedText(undefined);

  return (
    <div className="app">
      <nav className={`sidebar ${sidebarExpanded ? "sidebar-expanded" : "sidebar-compact"}`}>
        {/* Header: logo or expand button */}
        <div className="sidebar-header">
          {sidebarExpanded ? (
            <>
              <span className="sidebar-logo-text">DeepM</span>
              <button className="sidebar-toggle" onClick={toggleSidebar} title="Collapse sidebar">
                <ChevronLeft size={15} />
              </button>
            </>
          ) : (
            <button className="sidebar-toggle sidebar-toggle-expand" onClick={toggleSidebar} title="Expand sidebar">
              <ChevronRight size={15} />
            </button>
          )}
        </div>

        {/* Navigation */}
        <div className="sidebar-nav">
          <NavBtn active={view === "translator"} onClick={() => setView("translator")}
            icon={<Languages size={18} />} label={t.nav_translate} expanded={sidebarExpanded} />
          <NavBtn active={view === "history"} onClick={() => setView("history")}
            icon={<History size={18} />} label={t.nav_history} expanded={sidebarExpanded} />
          <NavBtn active={view === "model_manager"} onClick={() => setView("model_manager")}
            icon={<Package size={18} />} label={t.nav_model} expanded={sidebarExpanded} />
          <NavBtn active={view === "settings"} onClick={() => setView("settings")}
            icon={<Settings size={18} />} label={t.nav_settings} expanded={sidebarExpanded} />
        </div>

        {/* Status */}
        <div className="sidebar-bottom">
          <div className={`model-status-dot ${modelReady ? "ready" : "not-ready"}`}
            title={modelReady ? t.model_ready : t.no_model} />
          {sidebarExpanded && (
            <span className="model-status-label">{modelReady ? t.model_ready : t.no_model}</span>
          )}
        </div>
      </nav>

      <main className="main-content">
        {translateReplaceActive && (
          <div className="translate-replace-banner">
            <span className="tr-spinner" /> {t.translating_in_place}
          </div>
        )}

        {view === "translator" && (
          <TranslatorPanel
            glossaryEntries={glossary}
            initialText={injectedText ?? historyEntry?.source_text}
            onInitialTextConsumed={handleInjectedConsumed}
            defaultSourceLang={defaultSourceLang}
            defaultTargetLang={defaultTargetLang}
            onTranslated={(sl, tl, st, tt) => {
              if (historyEntry) setHistoryEntry(null);
            }}
          />
        )}
        {view === "history" && <HistoryPanel onSelect={handleHistorySelect} />}
        {view === "model_manager" && <ModelManager onModelReady={handleModelReady} />}
        {view === "onboarding" && <ModelManager onModelReady={handleModelReady} isOnboarding />}
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

function NavBtn({
  active, onClick, icon, label, expanded,
}: {
  active: boolean; onClick: () => void; icon: React.ReactNode; label: string; expanded: boolean;
}) {
  return (
    <button
      className={`nav-btn ${active ? "active" : ""} ${expanded ? "nav-btn-full" : ""}`}
      onClick={onClick}
      title={label}
    >
      <span className="nav-icon">{icon}</span>
      {expanded && <span className="nav-label">{label}</span>}
    </button>
  );
}
