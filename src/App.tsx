import React, { useState, useEffect, useCallback } from "react";
import {
  Languages, History, Package, Settings, Info,
  ChevronRight, ChevronLeft,
} from "lucide-react";
import type { AppView, TranslationHistoryEntry } from "./types";
import type { Locale } from "./i18n";
import { I18nProvider, useI18n } from "./i18n-context";
import TranslatorPanel from "./components/TranslatorPanel";
import ModelManager from "./components/ModelManager";
import HistoryPanel from "./components/HistoryPanel";
import SettingsPanel from "./components/SettingsPanel";
import OcrTestPanel from "./components/OcrTestPanel";
import AboutPanel from "./components/AboutPanel";
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
  const [defaultTargetLang, setDefaultTargetLang] = useState("auto");
  const [defaultSourceLang, setDefaultSourceLang] = useState("auto");
  const [autoTargetPriority, setAutoTargetPriority] = useState("ru");

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

  // Poll the real engine status forever so the status dot always reflects
  // the actual state — fast at startup, then a steady heartbeat so the dot
  // turns red if the engine dies and green again if it recovers.
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    let started = false;

    const check = () => {
      if (cancelled) return;
      getModelStatus()
        .then((s) => {
          if (cancelled) return;
          setModelReady(s.type === "ready");
        })
        .catch(() => {
          if (!cancelled) setModelReady(false);
        })
        .finally(() => {
          if (cancelled) return;
          // Probe every 4s for the first minute, then every 15s.
          const delay = started ? 15000 : 4000;
          started = true;
          timer = setTimeout(check, delay);
        });
    };

    check();
    return () => { cancelled = true; clearTimeout(timer); };
  }, []);

  // Engine lifecycle events flip the dot immediately, without waiting for a poll.
  useEffect(() => {
    const subs = [
      listen("model_ready", () => setModelReady(true)),
      listen("model_error", () => setModelReady(false)),
    ];
    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, []);

  // Settings load — re-read on navigation so edits in the Settings panel
  // (glossary, default langs, auto-target priority) take effect immediately.
  useEffect(() => {
    getSettings().then((s) => {
      setGlossary(s.glossary);
      setDefaultTargetLang(s.default_target_lang);
      setDefaultSourceLang(s.default_source_lang);
      setAutoTargetPriority(s.auto_target_priority ?? "ru");
    }).catch(() => {});
  }, [view]);

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

  // Esc returns to the translator from any secondary view (history, model
  // manager, settings, about, ocr test). Onboarding is left alone — there's
  // nowhere to go back to until a model is ready.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      setView((cur) => (cur === "onboarding" || cur === "translator" ? cur : "translator"));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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

        {/* About + status */}
        <div className="sidebar-bottom">
          <NavBtn active={view === "about"} onClick={() => setView("about")}
            icon={<Info size={18} />} label={t.nav_about} expanded={sidebarExpanded} />
          <div className="sidebar-status">
            <div className={`model-status-dot ${modelReady ? "ready" : "not-ready"}`}
              title={modelReady ? t.model_ready : t.no_model} />
            {sidebarExpanded && (
              <span className="model-status-label">{modelReady ? t.model_ready : t.no_model}</span>
            )}
          </div>
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
            autoTargetPriority={autoTargetPriority}
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
        {/* OCR Test Mode kept for diagnostics; no UI entry point (re-add a button to open). */}
        {view === "ocr_test" && <OcrTestPanel onBack={() => setView("settings")} />}
        {view === "about" && <AboutPanel />}
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
