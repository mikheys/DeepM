import React, { useState } from "react";
import { ArrowLeft, ImagePlus } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { ocrTest } from "../api";
import type { OcrTestResult } from "../types";
import { useI18n } from "../i18n-context";
import "./OcrTestPanel.css";

type Props = { onBack: () => void };

/**
 * Developer-facing OCR comparison: run both engines on one image and show raw
 * vs normalized text, timing, model and preprocessing side by side. No "best
 * result" picking — just data for choosing models/preprocessing.
 */
export default function OcrTestPanel({ onBack }: Props) {
  const { t } = useI18n();
  const [path, setPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<OcrTestResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pickAndRun = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff"] }],
    }).catch(() => null);
    if (!file || typeof file !== "string") return;
    setPath(file);
    setResults(null);
    setError(null);
    setBusy(true);
    try {
      setResults(await ocrTest(file));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
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
        {path && <span className="ocr-test-path" title={path}>{path}</span>}
      </div>

      {busy && <div className="ocr-test-status">{t.ocr_working}</div>}
      {error && <div className="ocr-test-status ocr-test-error">{error}</div>}

      {results && (
        <div className="ocr-test-grid">
          {results.map((r) => (
            <div className="ocr-test-card" key={r.engine}>
              <div className="ocr-test-card-head">
                <span className="ocr-test-engine">{r.engine}</span>
                <span className="ocr-test-ms">{r.ms} ms</span>
              </div>
              <div className="ocr-test-meta">
                <span>{r.model}</span>
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
                  <div className="ocr-test-block">
                    <div className="ocr-test-block-label">{t.ocr_test_normalized}</div>
                    <pre className="ocr-test-text">{r.normalized || "—"}</pre>
                  </div>
                </>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
