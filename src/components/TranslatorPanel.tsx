import React, { useState, useCallback, useRef, useEffect } from "react";
import { LANGUAGES, TARGET_LANGUAGES, type Language } from "../types";
import { translate, detectLanguage } from "../api";
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

export default function TranslatorPanel({ glossaryEntries = [], onTranslated, initialText, onInitialTextConsumed, defaultSourceLang, defaultTargetLang }: Props) {
  const [sourceText, setSourceText] = useState(initialText ?? "");
  const [translatedText, setTranslatedText] = useState("");
  const [sourceLang, setSourceLang] = useState(defaultSourceLang ?? "auto");
  const [targetLang, setTargetLang] = useState(defaultTargetLang ?? "en");
  const [detectedLang, setDetectedLang] = useState<string | null>(null);
  const [isTranslating, setIsTranslating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [charCount, setCharCount] = useState(0);
  const [splitRatio, setSplitRatio] = useState(50);
  const [layout, setLayout] = useState<"horizontal" | "vertical">("horizontal");
  const [prevContext, setPrevContext] = useState<string | null>(null);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const isDragging = useRef(false);

  // Apply default language changes when settings load asynchronously
  useEffect(() => {
    if (defaultSourceLang) setSourceLang(defaultSourceLang);
  }, [defaultSourceLang]);

  useEffect(() => {
    if (defaultTargetLang) setTargetLang(defaultTargetLang);
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
      try {
        const relevantGlossary = glossaryEntries.filter(
          (e) => e.lang_pair === `${src}->${tgt}` || e.lang_pair === "auto"
        );
        const result = await translate({
          source_text: text,
          source_lang: src,
          target_lang: tgt,
          context: prevContext ?? undefined,
          glossary_entries: relevantGlossary.length > 0 ? relevantGlossary : undefined,
        });
        setTranslatedText(result.translated_text);
        if (result.detected_lang) setDetectedLang(result.detected_lang);
        onTranslated?.(src, tgt, text, result.translated_text);
        setPrevContext(text.slice(-500));
      } catch (e) {
        setError(String(e));
      } finally {
        setIsTranslating(false);
      }
    },
    [glossaryEntries, onTranslated, prevContext]
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
    const prevSrc = sourceLang;
    const prevTgt = targetLang;
    setSourceLang(prevTgt);
    setTargetLang(prevSrc);
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
    setSourceText("");
    setTranslatedText("");
    setCharCount(0);
    setDetectedLang(null);
    setPrevContext(null);
  };

  // Drag-to-resize divider
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

  const sourceLangLabel = () => {
    if (sourceLang === "auto") {
      const found = detectedLang
        ? LANGUAGES.find((l) => l.code === detectedLang)
        : null;
      return found ? `Auto (${found.nativeName})` : "Auto-detect";
    }
    return LANGUAGES.find((l) => l.code === sourceLang)?.nativeName ?? sourceLang;
  };

  return (
    <div
      className={`translator-panel layout-${layout}`}
      ref={containerRef}
      style={
        layout === "horizontal"
          ? ({ "--split": `${splitRatio}%` } as React.CSSProperties)
          : ({ "--split": `${splitRatio}%` } as React.CSSProperties)
      }
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
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
          {detectedLang && sourceLang === "auto" && (
            <span className="detected-badge">
              {LANGUAGES.find((l) => l.code === detectedLang)?.nativeName}
            </span>
          )}
          <div className="toolbar-spacer" />
          {sourceText && (
            <button className="icon-btn" onClick={handleClearSource} title="Clear">
              ✕
            </button>
          )}
        </div>
        <textarea
          className="pane-textarea"
          placeholder="Enter text to translate…"
          value={sourceText}
          onChange={handleSourceChange}
          autoFocus
        />
        <div className="pane-footer">
          <span className="char-count">{charCount > 0 ? `${charCount} chars` : ""}</span>
        </div>
      </div>

      {/* Divider */}
      <div
        className={`divider divider-${layout}`}
        onMouseDown={startDrag}
      >
        <button
          className="swap-btn"
          onClick={handleSwapLangs}
          disabled={sourceLang === "auto"}
          title="Swap languages"
        >
          {layout === "horizontal" ? "⇄" : "⇅"}
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
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
          <div className="toolbar-spacer" />
          <button
            className="layout-toggle icon-btn"
            onClick={() => setLayout(layout === "horizontal" ? "vertical" : "horizontal")}
            title="Toggle layout"
          >
            {layout === "horizontal" ? "⊟" : "⊞"}
          </button>
          {translatedText && (
            <button className="icon-btn" onClick={handleCopyTranslation} title="Copy translation">
              ⎘
            </button>
          )}
        </div>
        <div className="pane-textarea output-area">
          {isTranslating ? (
            <span className="translating-indicator">Translating…</span>
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
