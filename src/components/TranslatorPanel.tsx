import React, { useState, useCallback, useRef, useEffect } from "react";
import { ArrowLeftRight, ArrowUpDown, Columns2, LayoutPanelTop, Copy, X } from "lucide-react";
import { LANGUAGES, TARGET_LANGUAGES } from "../types";
import { translate, detectLanguage } from "../api";
import { useI18n } from "../i18n-context";
import "./TranslatorPanel.css";

type Props = {
  glossaryEntries?: { source: string; target: string; lang_pair: string }[];
  onTranslated?: (
    sourceLang: string,
    targetLang: string,
    sourceText: string,
    translatedText: string
  ) => void;
  initialText?: string;
  onInitialTextConsumed?: () => void;
  defaultSourceLang?: string;
  defaultTargetLang?: string;
};

type TranslationMode = "standard" | "contextual" | "formatted";

/** Choose the "opposite primary" language: en↔ru, others → en */
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
  const [targetLang, setTargetLang] = useState(defaultTargetLang ?? "ru");
  /** When true, target follows detected source automatically */
  const [autoTarget, setAutoTarget] = useState(true);
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

  useEffect(() => {
    if (defaultTargetLang) {
      setTargetLang(defaultTargetLang);
      setAutoTarget(false); // user explicitly configured a default
    }
  }, [defaultTargetLang]);

  useEffect(() => {
    if (initialText) {
      setSourceText(initialText);
      scheduleTranslate(initialText, sourceLang, targetLang);
      onInitialTextConsumed?.();
    }
  }, [initialText]);

  const runTranslate = useCallback(
    async (text: string, src: string, tgt: string) => {
      if (!text.trim()) {
        setTranslatedText("");
        setDetectedLang(null);
        return;
      }
      setIsTranslating(true);
      setError(null);

      // Auto-detect source and choose opposite target if autoTarget is on
      let resolvedSrc = src;
      let resolvedTgt = tgt;
      if (src === "auto") {
        try {
          const detected = await detectLanguage(text);
          setDetectedLang(detected);
          resolvedSrc = detected;
          if (autoTarget) {
            resolvedTgt = oppositePrimary(detected);
            setTargetLang(resolvedTgt);
          }
        } catch { /* keep defaults */ }
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
    [glossaryEntries, onTranslated, prevContext, mode, autoTarget]
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
    if (sourceLang === "auto") return;
    setSourceLang(targetLang);
    setTargetLang(sourceLang);
    setSourceText(translatedText);
    setTranslatedText(sourceText);
    setCharCount(translatedText.length);
    setAutoTarget(false);
  };

  const handleSourceLangChange = (code: string) => {
    setSourceLang(code);
    scheduleTranslate(sourceText, code, targetLang);
  };

  const handleTargetLangChange = (code: string) => {
    setTargetLang(code);
    setAutoTarget(false); // user manually chose target → disable auto
    scheduleTranslate(sourceText, sourceLang, code);
  };

  const handleCopyTranslation = () => {
    if (translatedText) navigator.clipboard.writeText(translatedText);
  };

  const handleClearSource = () => {
    setSourceText("");
    setTranslatedText("");
    setCharCount(0);
    setDetectedLang(null);
    setPrevContext(null);
  };

  // Drag-to-resize divider — works on the divider AND on the handle button
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

  const MODE_OPTIONS: { value: TranslationMode; label: string }[] = [
    { value: "standard", label: t.mode_standard },
    { value: "contextual", label: t.mode_contextual },
    { value: "formatted", label: t.mode_formatted },
  ];

  return (
    <div
      className={`translator-panel layout-${layout}`}
      ref={containerRef}
      style={{ "--split": `${splitRatio}%` } as React.CSSProperties}
    >
      {/* Source pane */}
      <div className="pane pane-source">
        <div className="pane-toolbar">
          <select
            className="lang-select"
            value={sourceLang}
            onChange={(e) => handleSourceLangChange(e.target.value)}
          >
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>{l.name}</option>
            ))}
          </select>
          {detectedBadge && sourceLang === "auto" && (
            <span className="detected-badge">{detectedBadge}</span>
          )}
          <div className="toolbar-spacer" />
          <select
            className="mode-select"
            value={mode}
            onChange={(e) => setMode(e.target.value as TranslationMode)}
            title={t.mode_hint}
          >
            {MODE_OPTIONS.map((m) => (
              <option key={m.value} value={m.value}>{m.label}</option>
            ))}
          </select>
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
          <span className="char-count">{charCount > 0 ? t.chars(charCount) : ""}</span>
        </div>
      </div>

      {/* Divider with integrated handle */}
      <div
        className={`divider divider-${layout}`}
        onMouseDown={startDrag}
      >
        <button
          className={`divider-handle ${sourceLang === "auto" ? "divider-handle-disabled" : ""}`}
          onMouseDown={startDrag}
          onClick={(e) => { e.stopPropagation(); handleSwapLangs(); }}
          title={t.swap_langs}
        >
          {layout === "horizontal"
            ? <ArrowLeftRight size={13} strokeWidth={2} />
            : <ArrowUpDown size={13} strokeWidth={2} />
          }
        </button>
      </div>

      {/* Target pane */}
      <div className="pane pane-target">
        <div className="pane-toolbar">
          <select
            className="lang-select"
            value={targetLang}
            onChange={(e) => handleTargetLangChange(e.target.value)}
          >
            {TARGET_LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>{l.name}</option>
            ))}
          </select>
          {autoTarget && sourceLang === "auto" && (
            <span className="auto-target-badge" title="Auto-selected based on source">auto</span>
          )}
          <div className="toolbar-spacer" />
          <button
            className="icon-btn"
            onClick={() => setLayout(layout === "horizontal" ? "vertical" : "horizontal")}
            title="Toggle layout"
          >
            {layout === "horizontal"
              ? <LayoutPanelTop size={15} />
              : <Columns2 size={15} />
            }
          </button>
          {translatedText && (
            <button className="icon-btn" onClick={handleCopyTranslation} title={t.copy_translation}>
              <Copy size={15} />
            </button>
          )}
        </div>
        <div className="pane-textarea output-area">
          {isTranslating ? (
            <span className="translating-indicator">{t.translating}</span>
          ) : error ? (
            <span className="error-text">{error}</span>
          ) : (
            <span className="output-text">{translatedText}</span>
          )}
        </div>
        <div className="pane-footer" />
      </div>
    </div>
  );
}
