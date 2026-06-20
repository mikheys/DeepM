import React, { useState } from "react";
import { ArrowLeft, ImagePlus, Copy, Check } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { ocrTestAll } from "../api";
import type { OcrTestResult } from "../types";
import { useI18n } from "../i18n-context";
import "./OcrTestPanel.css";

type Props = { onBack: () => void };

/**
 * Runs both engines across every preprocessing variant on one image and shows
 * raw vs normalized text, timing, model and preprocessing. The whole matrix can
 * be copied as plain text (so results can be shared without screenshots).
 */
export default function OcrTestPanel({ onBack }: Props) {
  const { t } = useI18n();
  const [path, setPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<OcrTestResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const pickAndRun = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff"] }],
    }).catch(() => null);
    if (!file || typeof file !== "string") return;
    setPath(file);
    setResults(null);
    setError(null);
    setCopied(false);
    setBusy(true);
    try {
      setResults(await ocrTestAll(file));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const buildReport = (): string => {
    if (!results) return "";
    let s = `OCR Test - ${path ?? ""}\n${"=".repeat(64)}\n`;
    let lastModel = "";
    for (const r of results) {
      if (r.model !== lastModel) {
        s += `\n### ${r.model || r.engine.toUpperCase()}\n`;
        lastModel = r.model;
      }
      s += `\n--- preprocess: ${r.preprocess} (${r.ms} ms) ---\n`;
      if (r.error) {
        s += `ERROR: ${r.error}\n`;
      } else {
        s += `RAW:\n${r.text || "(empty)"}\n`;
        if (r.normalized !== r.text) s += `NORMALIZED:\n${r.normalized || "(empty)"}\n`;
      }
    }
    return s;
  };

  const copyReport = async () => {
    try {
      await navigator.clipboard.writeText(buildReport());
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch { /* ignore */ }
  };

  return (
    <div className="ocr-test-panel">
      <div className="ocr-test-header">
        <button className="icon-btn" onClick={onBack} title={t.back}>
          <ArrowLeft size={16} />
        </button>
        <h2>{t.ocr_test_title}</h2>
      </div>

      <p className="ocr-test-intro">{t.ocr_test_intro}</p>

      <div className="ocr-test-actions">
        <button className="btn-primary" onClick={pickAndRun} disabled={busy}>
          <ImagePlus size={15} /> {t.ocr_test_pick}
        </button>
        {results && (
          <button className="btn-secondary" onClick={copyReport}>
            {copied ? <Check size={15} /> : <Copy size={15} />}
            {copied ? t.ocr_test_copied : t.ocr_test_copy}
          </button>
        )}
        {path && <span className="ocr-test-path" title={path}>{path}</span>}
      </div>

      {busy && <div className="ocr-test-status">{t.ocr_test_running_all}</div>}
      {error && <div className="ocr-test-status ocr-test-error">{error}</div>}

      {results && (
        <div className="ocr-test-grid">
          {results.map((r, i) => (
            <div className="ocr-test-card" key={`${r.engine}-${r.preprocess}-${i}`}>
              <div className="ocr-test-card-head">
                <span className="ocr-test-engine">{r.model || r.engine}</span>
                <span className="ocr-test-ms">{r.ms} ms</span>
              </div>
              <div className="ocr-test-meta">
                <span>{t.ocr_test_prep}: {r.preprocess}</span>
              </div>
              {r.error ? (
                <div className="ocr-test-status ocr-test-error">{r.error}</div>
              ) : (
                <>
                  <div className="ocr-test-block">
                    <div className="ocr-test-block-label">{t.ocr_test_raw}</div>
                    <pre className="ocr-test-text">{r.text || "—"}</pre>
                  </div>
                  {r.normalized !== r.text && (
                    <div className="ocr-test-block">
                      <div className="ocr-test-block-label">{t.ocr_test_normalized}</div>
                      <pre className="ocr-test-text">{r.normalized || "—"}</pre>
                    </div>
                  )}
                </>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
