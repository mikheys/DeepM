import React, { useState, useCallback, useRef, useEffect } from "react";
import {
  ArrowLeftRight, ArrowUpDown,
  Columns2, Rows2,
  Copy, X,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { ScanLine, ImagePlus, ClipboardPaste } from "lucide-react";
import { LANGUAGES, TARGET_LANGUAGES } from "../types";
import {
  translate, detectLanguage, getModelStatus, getSettings,
  ocrStatus, ocrFromClipboard, ocrFromFile, launchSnip,
} from "../api";
import { useI18n } from "../i18n-context";
import "./TranslatorPanel.css";

// Which translation modes each model family supports.
function modelVersionFromPath(path: string): "Hy-MT2" | "HY-MT1.5" {
  const f = (path || "").toLowerCase();
  return f.includes("mt2") ? "Hy-MT2" : "HY-MT1.5";
}

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

type TranslationMode =
  | "standard" | "contextual" | "formatted"
  | "style" | "structured" | "delimiter";

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
  const [style, setStyle] = useState("");
  const [prevContext, setPrevContext] = useState<string | null>(null);
  const [modelVersion, setModelVersion] = useState<"Hy-MT2" | "HY-MT1.5">("HY-MT1.5");
  const [ocrAvailable, setOcrAvailable] = useState(true);
  const [ocrBusy, setOcrBusy] = useState(false);
  const [ocrError, setOcrError] = useState<string | null>(null);
  const [ocrEngine, setOcrEngine] = useState("windows");

  useEffect(() => {
    getSettings().then((s) => {
      const eng = s.ocr_engine || "windows";
      setOcrEngine(eng);
      return ocrStatus(eng);
    }).then(setOcrAvailable).catch(() => setOcrAvailable(false));
  }, []);

  // Detect the active model family so the mode list matches its capabilities.
  useEffect(() => {
    const detect = () => getModelStatus().then((s) => {
      if (s.type === "ready") setModelVersion(modelVersionFromPath((s as any).path ?? ""));
    }).catch(() => {});
    detect();
    const unsub = listen("model_ready", detect);
    return () => { unsub.then((f) => f()); };
  }, []);

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
          mode,
          style: mode === "style" ? style : undefined,
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
    [glossaryEntries, onTranslated, prevContext, mode, style]
  );

  const scheduleTranslate = useCallback(
    (text: string, src: string, tgt: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => runTranslate(text, src, tgt), 600);
    },
    [runTranslate]
  );

  // Re-translate when the mode changes (so switching mode takes effect at once).
  useEffect(() => {
    if (sourceText.trim()) scheduleTranslate(sourceText, sourceLang, targetLang);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

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

  // ── OCR: screenshot / image → source text ────────────────────────────
  const applyOcr = (text: string) => {
    const clean = text.trim();
    if (!clean) { setOcrError(t.ocr_empty); return; }
    setOcrError(null);
    setSourceText(clean);
    setCharCount(clean.length);
    scheduleTranslate(clean, sourceLang, targetLang);
  };
  const ocrErr = (e: unknown) => {
    const s = String(e);
    setOcrError(
      s.includes("no_ocr_language") ? t.ocr_no_lang
        : s.includes("tesseract_not_installed") ? t.ocr_tesseract_missing
        : s.includes("rapidocr_models_missing") ? t.ocr_rapidocr_no_models
        : s.includes("rapidocr_unavailable") ? t.ocr_rapidocr_soon
        : s.includes("no_image") ? t.ocr_no_image : s
    );
  };
  const handleClipImage = async () => {
    setOcrBusy(true); setOcrError(null);
    try { applyOcr(await ocrFromClipboard(ocrEngine)); } catch (e) { ocrErr(e); } finally { setOcrBusy(false); }
  };
  const handleFileImage = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff"] }],
    }).catch(() => null);
    if (!file || typeof file !== "string") return;
    setOcrBusy(true); setOcrError(null);
    try { applyOcr(await ocrFromFile(ocrEngine, file)); } catch (e) { ocrErr(e); } finally { setOcrBusy(false); }
  };
  const handleSnip = async () => {
    setOcrBusy(true); setOcrError(null);
    await launchSnip().catch(() => {});
    const start = Date.now();
    const poll = async () => {
      if (Date.now() - start > 30000) { setOcrBusy(false); return; }
      try {
        const text = await ocrFromClipboard(ocrEngine);
        if (text && text.trim()) { applyOcr(text); setOcrBusy(false); return; }
      } catch { /* snip not taken yet — keep waiting */ }
      window.setTimeout(poll, 800);
    };
    window.setTimeout(poll, 1500);
  };

  const ocrOverlay = !sourceText ? (
    <div className="ocr-overlay">
      <div className="ocr-actions">
        <button className="ocr-btn" onClick={handleSnip} disabled={ocrBusy || !ocrAvailable} title={t.ocr_snip_hint}>
          <ScanLine size={15} /> {t.ocr_snip}
        </button>
        <button className="ocr-btn" onClick={handleClipImage} disabled={ocrBusy || !ocrAvailable} title={t.ocr_clipboard_hint}>
          <ClipboardPaste size={15} /> {t.ocr_clipboard}
        </button>
        <button className="ocr-btn" onClick={handleFileImage} disabled={ocrBusy || !ocrAvailable} title={t.ocr_file_hint}>
          <ImagePlus size={15} /> {t.ocr_file}
        </button>
      </div>
      {ocrBusy && <div className="ocr-status">{t.ocr_working}</div>}
      {!ocrAvailable && <div className="ocr-status ocr-warn">{t.ocr_no_lang}</div>}
      {ocrError && <div className="ocr-status ocr-warn">{ocrError}</div>}
    </div>
  ) : null;

  // Minimum pixel width each header pane needs so its fixed-width controls
  // (lang select + buttons + badge) never get clipped. Source = select + clear;
  // target = swap + select + badge + copy.
  const MIN_SOURCE_PX = 200;
  const MIN_TARGET_PX = 250;

  // Clamp a horizontal split ratio so neither pane drops below its minimum,
  // both while dragging and on window resize.
  const clampSplit = (ratio: number): number => {
    const w = containerRef.current?.getBoundingClientRect().width ?? 0;
    if (w <= 0) return Math.max(20, Math.min(80, ratio));
    const minR = (MIN_SOURCE_PX / w) * 100;
    const maxR = 100 - (MIN_TARGET_PX / w) * 100;
    if (minR >= maxR) return (minR + maxR) / 2; // window too small to satisfy both
    return Math.max(minR, Math.min(maxR, ratio));
  };

  // Double-click the divider to snap it back to the centre (clamped to fit).
  const resetSplit = () => setSplitRatio(clampSplit(50));

  // Re-clamp when the window/container resizes so controls never disappear.
  useEffect(() => {
    const el = containerRef.current;
    if (!el || layout !== "horizontal") return;
    const ro = new ResizeObserver(() => setSplitRatio((r) => clampSplit(r)));
    ro.observe(el);
    return () => ro.disconnect();
  }, [layout]);

  const startDrag = (e: React.MouseEvent) => {
    e.preventDefault();
    isDragging.current = true;
    const onMove = (ev: MouseEvent) => {
      if (!containerRef.current || !isDragging.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      if (layout === "horizontal") {
        const ratio = ((ev.clientX - rect.left) / rect.width) * 100;
        setSplitRatio(clampSplit(ratio));
      } else {
        const ratio = ((ev.clientY - rect.top) / rect.height) * 100;
        setSplitRatio(Math.max(20, Math.min(80, ratio)));
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

  const MODE_OPTIONS: { value: TranslationMode; label: string }[] =
    modelVersion === "Hy-MT2"
      ? [
          { value: "standard",   label: t.mode_standard },
          { value: "contextual", label: t.mode_contextual },
          { value: "style",      label: t.mode_style },
          { value: "structured", label: t.mode_structured },
          { value: "delimiter",  label: t.mode_delimiter },
        ]
      : [
          { value: "standard",   label: t.mode_standard },
          { value: "contextual", label: t.mode_contextual },
          { value: "formatted",  label: t.mode_formatted },
        ];

  // If the active model doesn't support the current mode, fall back to standard.
  useEffect(() => {
    if (!MODE_OPTIONS.some((m) => m.value === mode)) setMode("standard");
  }, [modelVersion]); // eslint-disable-line react-hooks/exhaustive-deps

  const swapDisabled = sourceLang === "auto" || targetLang === "auto";

  // Copy lives in the top bar now — always present (disabled when empty) so it
  // never disappears when the target pane is narrow.
  const copyBtn = (
    <button
      className="icon-btn"
      onClick={handleCopyTranslation}
      disabled={!translatedText}
      title={t.copy_translation}
    >
      <Copy size={15} />
    </button>
  );

  // Single unified bottom bar (mode + char count + layout toggle), shared by
  // both layouts and always full-width — never split by the divider.
  const footer = (
    <div className="panel-footer">
      <div className="mode-control" title={t.mode_hint}>
        <span className="mode-label">{t.mode_label}:</span>
        <select className="mode-select" value={mode}
          onChange={(e) => setMode(e.target.value as TranslationMode)}>
          {MODE_OPTIONS.map((m) => (
            <option key={m.value} value={m.value}>{m.label}</option>
          ))}
        </select>
      </div>
      {mode === "style" && (
        <input
          className="style-input"
          placeholder={t.mode_style_placeholder}
          value={style}
          onChange={(e) => setStyle(e.target.value)}
          onBlur={() => { if (sourceText.trim()) scheduleTranslate(sourceText, sourceLang, targetLang); }}
        />
      )}
      <span className="char-count">{charCount > 0 ? t.chars(charCount) : ""}</span>
      <div className="toolbar-spacer" />
      <button
        className="icon-btn"
        onClick={() => setLayout(layout === "horizontal" ? "vertical" : "horizontal")}
        title={layout === "horizontal" ? t.layout_to_vertical : t.layout_to_horizontal}
      >
        {layout === "horizontal" ? <Rows2 size={15} /> : <Columns2 size={15} />}
      </button>
    </div>
  );

  const outputArea = (
    <div className="output-area">
      {isTranslating ? (
        <span className="translating-indicator">{t.translating}</span>
      ) : error ? (
        <span className="error-text">{error}</span>
      ) : (
        <span className="output-text">{translatedText}</span>
      )}
    </div>
  );

  const sourceLangSelect = (
    <select className="lang-select" value={sourceLang}
      onChange={(e) => handleSourceLangChange(e.target.value)}>
      {LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.name}</option>)}
    </select>
  );
  const targetLangSelect = (
    <select className="lang-select" value={targetLang}
      onChange={(e) => handleTargetLangChange(e.target.value)}>
      {TARGET_LANGUAGES.map((l) => <option key={l.code} value={l.code}>{l.name}</option>)}
    </select>
  );

  if (layout === "horizontal") {
    return (
      <div
        className="translator-panel"
        ref={containerRef}
        style={{ "--split": `${splitRatio}%` } as React.CSSProperties}
      >
        {/* ── Shared header row ───────────────────────── */}
        <div className="panel-header">
          <div className="panel-header-section panel-header-source">
            {sourceLangSelect}
            {detectedBadge && sourceLang === "auto" && (
              <span className="detected-badge">{detectedBadge}</span>
            )}
            <div className="toolbar-spacer" />
            <button className="icon-btn" onClick={handleClearSource}
              disabled={!sourceText} title={t.clear}>
              <X size={14} />
            </button>
          </div>

          <div className="panel-divider-col" />

          <div className="panel-header-section panel-header-target">
            <button className="icon-btn swap-btn" onClick={handleSwapLangs}
              disabled={swapDisabled} title={t.swap_langs}>
              <ArrowLeftRight size={14} strokeWidth={2} />
            </button>
            {targetLangSelect}
            {resolvedTarget && (
              <span className="auto-target-badge">→ {resolvedTarget.toUpperCase()}</span>
            )}
            <div className="toolbar-spacer" />
            {copyBtn}
          </div>
        </div>

        {/* ── Body with draggable divider ──────────────── */}
        <div className="panel-body">
          <div className="pane-body pane-body-source">
            <div className="textarea-wrap">
              <textarea
                className="pane-textarea"
                placeholder={t.source_placeholder}
                value={sourceText}
                onChange={handleSourceChange}
                autoFocus
              />
              {ocrOverlay}
            </div>
          </div>
          <div className="divider divider-h" onMouseDown={startDrag}
            onDoubleClick={resetSplit} title={t.divider_reset_hint} />
          <div className="pane-body pane-body-target">{outputArea}</div>
        </div>

        {footer}
      </div>
    );
  }

  // ── Vertical layout — shares the same unified footer ──────────────────
  return (
    <div
      className="translator-panel translator-vertical"
      ref={containerRef}
      style={{ "--split": `${splitRatio}%` } as React.CSSProperties}
    >
      <div className="pane-v pane-v-source">
        <div className="pane-toolbar">
          {sourceLangSelect}
          {detectedBadge && sourceLang === "auto" && (
            <span className="detected-badge">{detectedBadge}</span>
          )}
          <div className="toolbar-spacer" />
          <button className="icon-btn" onClick={handleClearSource}
            disabled={!sourceText} title={t.clear}>
            <X size={14} />
          </button>
        </div>
        <div className="textarea-wrap">
          <textarea
            className="pane-textarea"
            placeholder={t.source_placeholder}
            value={sourceText}
            onChange={handleSourceChange}
            autoFocus
          />
          {ocrOverlay}
        </div>
      </div>

      <div className="divider divider-v" onMouseDown={startDrag}
        onDoubleClick={resetSplit} title={t.divider_reset_hint} />

      <div className="pane-v pane-v-target">
        <div className="pane-toolbar">
          <button className="icon-btn swap-btn" onClick={handleSwapLangs}
            disabled={swapDisabled} title={t.swap_langs}>
            <ArrowUpDown size={14} strokeWidth={2} />
          </button>
          {targetLangSelect}
          {resolvedTarget && (
            <span className="auto-target-badge">→ {resolvedTarget.toUpperCase()}</span>
          )}
          <div className="toolbar-spacer" />
          {copyBtn}
        </div>
        {outputArea}
      </div>

      {footer}
    </div>
  );
}
