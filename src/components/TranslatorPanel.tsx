import React, { useState, useCallback, useRef, useEffect } from "react";
import {
  ArrowLeftRight, ArrowUpDown,
  Columns2, Rows2,
  Copy, X,
} from "lucide-react";
import { LANGUAGES, TARGET_LANGUAGES } from "../types";
import { translate, detectLanguage } from "../api";
import { useI18n } from "../i18n-context";
import "./TranslatorPanel.css";

type Props = {
  glossaryEntries?: { source: string; target: string; lang_pair: string }[];
  onTranslated?: (
    sourceLang: string, targetLang: string,
    sourceText: string, translatedText: string,
  ) => void;
  initialText?: string;
  onInitialTextConsumed?: () => void;
  defaultSourceLang?: string;
  defaultTargetLang?: string;
};

type TranslationMode = "standard" | "contextual" | "formatted";

function oppositePrimary(lang: string): string {
  if (lang === "en") return "ru";
  if (lang === "ru") return "en";
  return "en";
}

export default function TranslatorPanel({
  glossaryEntries = [],
  onTranslated,
  initialText,
  onInitialTextConsumed,
  defaultSourceLang,
  defaultTargetLang,
}: Props) {
  const { t } = useI18n();
  const [sourceText, setSourceText] = useState(initialText ?? "");
  const [translatedText, setTranslatedText] = useState("");
  const [sourceLang, setSourceLang] = useState(defaultSourceLang ?? "auto");
  const [targetLang, setTargetLang] = useState(defaultTargetLang ?? "auto");
  const [detectedLang, setDetectedLang] = useState<string | null>(null);
  const [isTranslating, setIsTranslating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [charCount, setCharCount] = useState(0);
  const [splitRatio, setSplitRatio] = useState(50);
  const [layout, setLayout] = useState<"horizontal" | "vertical">("horizontal");
  const [mode, setMode] = useState<TranslationMode>("standard");
  const [prevContext, setPrevContext] = useState<string | null>(null);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const isDragging = useRef(false);

  useEffect(() => { if (defaultSourceLang) setSourceLang(defaultSourceLang); }, [defaultSourceLang]);
  useEffect(() => { if (defaultTargetLang) setTargetLang(defaultTargetLang); }, [defaultTargetLang]);

  useEffect(() => {
    if (initialText) {
      setSourceText(initialText);
      scheduleTranslate(initialText, sourceLang, targetLang);
      onInitialTextConsumed?.();
    }
  }, [initialText]);

  const runTranslate = useCallback(
    async (text: string, src: string, tgt: string) => {
      if (!text.trim()) { setTranslatedText(""); setDetectedLang(null); return; }
      setIsTranslating(true);
      setError(null);

      let resolvedSrc = src;
      let resolvedTgt = tgt;
      if (src === "auto") {
        try {
          const detected = await detectLanguage(text);
          setDetectedLang(detected);
          resolvedSrc = detected;
        } catch { /* keep defaults */ }
      }
      if (resolvedTgt === "auto") {
        resolvedTgt = oppositePrimary(resolvedSrc);
      }

      try {
        const relevantGlossary = glossaryEntries.filter(
          (e) => e.lang_pair === `${resolvedSrc}->${resolvedTgt}` || e.lang_pair === "auto"
        );
        const result = await translate({
          source_text: text,
          source_lang: resolvedSrc,
          target_lang: resolvedTgt,
          context: mode === "contextual" ? (prevContext ?? undefined) : undefined,
          glossary_entries: relevantGlossary.length > 0 ? relevantGlossary : undefined,
          formatted: mode === "formatted",
        });
        setTranslatedText(result.translated_text);
        if (result.detected_lang) setDetectedLang(result.detected_lang);
        onTranslated?.(resolvedSrc, resolvedTgt, text, result.translated_text);
        if (mode === "contextual") setPrevContext(text.slice(-500));
      } catch (e) {
        setError(String(e));
      } finally {
        setIsTranslating(false);
      }
    },
    [glossaryEntries, onTranslated, prevContext, mode]
  );

  const scheduleTranslate = useCallback(
    (text: string, src: string, tgt: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => runTranslate(text, src, tgt), 600);
    },
    [runTranslate]
  );

  const handleSourceChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setSourceText(text);
    setCharCount(text.length);
    scheduleTranslate(text, sourceLang, targetLang);
  };

  const handleSwapLangs = () => {
    if (sourceLang === "auto" || targetLang === "auto") return;
    setSourceLang(targetLang);
    setTargetLang(sourceLang);
    setSourceText(translatedText);
    setTranslatedText(sourceText);
    setCharCount(translatedText.length);
  };

  const handleSourceLangChange = (code: string) => {
    setSourceLang(code);
    scheduleTranslate(sourceText, code, targetLang);
  };

  const handleTargetLangChange = (code: string) => {
    setTargetLang(code);
    scheduleTranslate(sourceText, sourceLang, code);
  };

  const handleCopyTranslation = () => {
    if (translatedText) navigator.clipboard.writeText(translatedText);
  };

  const handleClearSource = () => {
    setSourceText(""); setTranslatedText("");
    setCharCount(0); setDetectedLang(null); setPrevContext(null);
  };

  // Double-click the divider to snap it back to the centre (50/50).
  const resetSplit = () => setSplitRatio(50);

  const startDrag = (e: React.MouseEvent) => {
    e.preventDefault();
    isDragging.current = true;
    const onMove = (ev: MouseEvent) => {
      if (!containerRef.current || !isDragging.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      if (layout === "horizontal") {
        const ratio = ((ev.clientX - rect.left) / rect.width) * 100;
        setSplitRatio(Math.max(25, Math.min(75, ratio)));
      } else {
        const ratio = ((ev.clientY - rect.top) / rect.height) * 100;
        setSplitRatio(Math.max(25, Math.min(75, ratio)));
      }
    };
    const onUp = () => {
      isDragging.current = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  const detectedBadge = detectedLang
    ? LANGUAGES.find((l) => l.code === detectedLang)?.nativeName
    : null;

  // Show resolved target when "auto" is selected
  const resolvedTarget = targetLang === "auto" && detectedLang
    ? oppositePrimary(detectedLang)
    : null;

  const MODE_OPTIONS: { value: TranslationMode; label: string }[] = [
    { value: "standard",   label: t.mode_standard },
    { value: "contextual", label: t.mode_contextual },
    { value: "formatted",  label: t.mode_formatted },
  ];

  const swapDisabled = sourceLang === "auto" || targetLang === "auto";

  if (layout === "horizontal") {
    return (
      <div
        className="translator-panel"
        ref={containerRef}
        style={{ "--split": `${splitRatio}%` } as React.CSSProperties}
      >
        {/* ── Shared header row ───────────────────────── */}
        <div className="panel-header">
          {/* Source controls */}
          <div className="panel-header-section panel-header-source">
            <select className="lang-select" value={sourceLang}
              onChange={(e) => handleSourceLangChange(e.target.value)}>
              {LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.name}</option>)}
            </select>
            {detectedBadge && sourceLang === "auto" && (
              <span className="detected-badge">{detectedBadge}</span>
            )}
            <div className="toolbar-spacer" />
            {sourceText && (
              <button className="icon-btn" onClick={handleClearSource} title={t.clear}>
                <X size={14} />
              </button>
            )}
          </div>

          {/* Divider column spacer */}
          <div className="panel-divider-col" />

          {/* Target controls */}
          <div className="panel-header-section panel-header-target">
            <button
              className="icon-btn swap-btn"
              onClick={handleSwapLangs}
              disabled={swapDisabled}
              title={t.swap_langs}
            >
              <ArrowLeftRight size={14} strokeWidth={2} />
            </button>
            <select className="lang-select" value={targetLang}
              onChange={(e) => handleTargetLangChange(e.target.value)}>
              {TARGET_LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.name}</option>)}
            </select>
            {resolvedTarget && (
              <span className="auto-target-badge">→ {resolvedTarget.toUpperCase()}</span>
            )}
          </div>
        </div>

        {/* ── Body with draggable divider ──────────────── */}
        <div className="panel-body">
          <div className="pane-body pane-body-source">
            <textarea
              className="pane-textarea"
              placeholder={t.source_placeholder}
              value={sourceText}
              onChange={handleSourceChange}
              autoFocus
            />
          </div>
          <div className="divider divider-h" onMouseDown={startDrag}
            onDoubleClick={resetSplit} title={t.divider_reset_hint} />
          <div className="pane-body pane-body-target">
            <div className="output-area">
              {isTranslating ? (
                <span className="translating-indicator">{t.translating}</span>
              ) : error ? (
                <span className="error-text">{error}</span>
              ) : (
                <span className="output-text">{translatedText}</span>
              )}
            </div>
          </div>
        </div>

        {/* ── Shared footer row ───────────────────────── */}
        <div className="panel-footer">
          {/* Source footer */}
          <div className="panel-footer-section panel-footer-source">
            <div className="mode-control" title={t.mode_hint}>
              <span className="mode-label">{t.mode_label}:</span>
              <select className="mode-select" value={mode}
                onChange={(e) => setMode(e.target.value as TranslationMode)}>
                {MODE_OPTIONS.map((m) => (
                  <option key={m.value} value={m.value}>{m.label}</option>
                ))}
              </select>
            </div>
            <span className="char-count">{charCount > 0 ? t.chars(charCount) : ""}</span>
          </div>

          {/* Divider column spacer */}
          <div className="panel-divider-col" />

          {/* Target footer */}
          <div className="panel-footer-section panel-footer-target">
            <div className="toolbar-spacer" />
            <button
              className="icon-btn"
              onClick={() => setLayout("vertical")}
              title="Switch to vertical layout"
            >
              <Rows2 size={15} />
            </button>
            {translatedText && (
              <button className="icon-btn" onClick={handleCopyTranslation} title={t.copy_translation}>
                <Copy size={15} />
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }

  // ── Vertical layout ───────────────────────────────────
  return (
    <div
      className="translator-panel"
      ref={containerRef}
      style={{ "--split": `${splitRatio}%` } as React.CSSProperties}
    >
      <div className="pane-v pane-v-source">
        <div className="pane-toolbar">
          <select className="lang-select" value={sourceLang}
            onChange={(e) => handleSourceLangChange(e.target.value)}>
            {LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.name}</option>)}
          </select>
          {detectedBadge && sourceLang === "auto" && (
            <span className="detected-badge">{detectedBadge}</span>
          )}
          <div className="toolbar-spacer" />
          {sourceText && (
            <button className="icon-btn" onClick={handleClearSource} title={t.clear}>
              <X size={14} />
            </button>
          )}
        </div>
        <textarea
          className="pane-textarea"
          placeholder={t.source_placeholder}
          value={sourceText}
          onChange={handleSourceChange}
          autoFocus
        />
        <div className="pane-footer">
          <div className="mode-control" title={t.mode_hint}>
            <span className="mode-label">{t.mode_label}:</span>
            <select className="mode-select" value={mode}
              onChange={(e) => setMode(e.target.value as TranslationMode)}>
              {MODE_OPTIONS.map((m) => (
                <option key={m.value} value={m.value}>{m.label}</option>
              ))}
            </select>
          </div>
          <span className="char-count">{charCount > 0 ? t.chars(charCount) : ""}</span>
        </div>
      </div>

      <div className="divider divider-v" onMouseDown={startDrag}
        onDoubleClick={resetSplit} title={t.divider_reset_hint} />

      <div className="pane-v pane-v-target">
        <div className="pane-toolbar">
          <button
            className="icon-btn swap-btn"
            onClick={handleSwapLangs}
            disabled={swapDisabled}
            title={t.swap_langs}
          >
            <ArrowUpDown size={14} strokeWidth={2} />
          </button>
          <select className="lang-select" value={targetLang}
            onChange={(e) => handleTargetLangChange(e.target.value)}>
            {TARGET_LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.name}</option>)}
          </select>
          {resolvedTarget && (
            <span className="auto-target-badge">→ {resolvedTarget.toUpperCase()}</span>
          )}
        </div>
        <div className="output-area">
          {isTranslating ? (
            <span className="translating-indicator">{t.translating}</span>
          ) : error ? (
            <span className="error-text">{error}</span>
          ) : (
            <span className="output-text">{translatedText}</span>
          )}
        </div>
        <div className="pane-footer">
          <div className="toolbar-spacer" />
          <button
            className="icon-btn"
            onClick={() => setLayout("horizontal")}
            title="Switch to horizontal layout"
          >
            <Columns2 size={15} />
          </button>
          {translatedText && (
            <button className="icon-btn" onClick={handleCopyTranslation} title={t.copy_translation}>
              <Copy size={15} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
