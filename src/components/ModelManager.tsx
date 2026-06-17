import React, { useState, useEffect } from "react";
import type { ModelSize, Quantization, ModelStatus } from "../types";
import {
  getModelStatus,
  startModelDownload,
  cancelModelDownload,
  restartEngine,
  onDownloadProgress,
  onModelReady,
  onModelError,
} from "../api";
import "./ModelManager.css";

type Props = {
  onModelReady: () => void;
  isOnboarding?: boolean;
};

const MODEL_SIZES: { value: ModelSize; label: string; description: string; vram: string }[] = [
  {
    value: "1.8B",
    label: "HY-MT1.5-1.8B",
    description: "Recommended — fast, runs on most hardware",
    vram: "~1 GB RAM",
  },
  {
    value: "7B",
    label: "HY-MT1.5-7B",
    description: "Higher quality, requires more resources",
    vram: "~5–6 GB RAM",
  },
];

const QUANTIZATIONS: { value: Quantization; label: string; size: string }[] = [
  { value: "Q4_K_M", label: "Q4_K_M (recommended)", size: "~1.1 GB for 1.8B" },
  { value: "Q6_K",   label: "Q6_K (better quality)", size: "~1.5 GB for 1.8B" },
  { value: "Q8_0",   label: "Q8_0 (best quality)",   size: "~1.9 GB for 1.8B" },
];

export default function ModelManager({ onModelReady: onReady, isOnboarding }: Props) {
  const [status, setStatus] = useState<ModelStatus>({ type: "not_downloaded" });
  const [selectedSize, setSelectedSize] = useState<ModelSize>("1.8B");
  const [selectedQuant, setSelectedQuant] = useState<Quantization>("Q4_K_M");
  const [modelPath, setModelPath] = useState("");
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadSpeed, setDownloadSpeed] = useState(0);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [engineRetrying, setEngineRetrying] = useState(false);

  useEffect(() => {
    getModelStatus().then((s) => {
      setStatus(s);
    }).catch(() => {});

    const unsubProgress = onDownloadProgress((p, speed) => {
      setDownloadProgress(p);
      setDownloadSpeed(speed);
      setStatus({ type: "downloading", progress: p, speed_mbps: speed });
    });
    const unsubReady = onModelReady(() => {
      setStatus({ type: "ready", path: modelPath });
      setEngineError(null);
      onReady();
    });
    const unsubError = onModelError((msg) => {
      if (msg.includes("llama-server") || msg.includes("crashed") || msg.includes("start")) {
        setEngineError(msg);
      } else {
        setStatus({ type: "error", message: msg });
      }
    });

    return () => {
      unsubProgress.then((f) => f());
      unsubReady.then((f) => f());
      unsubError.then((f) => f());
    };
  }, []);

  const handleRetryEngine = async () => {
    setEngineRetrying(true);
    setEngineError(null);
    try {
      await restartEngine();
    } catch (e) {
      setEngineError(String(e));
    } finally {
      setEngineRetrying(false);
    }
  };

  const handleDownload = async () => {
    setStatus({ type: "downloading", progress: 0, speed_mbps: 0 });
    setDownloadProgress(0);
    setEngineError(null);
    try {
      await startModelDownload(selectedSize, selectedQuant);
    } catch (e) {
      setStatus({ type: "error", message: String(e) });
    }
  };

  const handleCancel = async () => {
    await cancelModelDownload();
    setStatus({ type: "not_downloaded" });
    setDownloadProgress(0);
  };

  const isDownloading = status.type === "downloading";
  const isReady = status.type === "ready";

  return (
    <div className={`model-manager ${isOnboarding ? "onboarding-mode" : ""}`}>
      {isOnboarding && (
        <div className="onboarding-header">
          <h1 className="onboarding-title">Welcome to DeepM</h1>
          <p className="onboarding-subtitle">
            Download the local translation model to get started. Your translations
            stay on your device — no internet required after setup.
          </p>
        </div>
      )}

      {/* Engine error banner */}
      {engineError && (
        <div className="engine-error-card">
          <div className="engine-error-title">⚠ Engine failed to start</div>
          <div className="engine-error-detail">{engineError}</div>
          <button
            className="btn-primary engine-retry-btn"
            onClick={handleRetryEngine}
            disabled={engineRetrying}
          >
            {engineRetrying ? "Starting…" : "Retry"}
          </button>
        </div>
      )}

      {/* Download progress */}
      {isDownloading && (
        <div className="download-progress-card">
          <div className="progress-label">
            <span>Downloading model…</span>
            <span className="progress-speed">{downloadSpeed.toFixed(1)} MB/s</span>
          </div>
          <div className="progress-bar-track">
            <div className="progress-bar-fill" style={{ width: `${downloadProgress}%` }} />
          </div>
          <div className="progress-pct">{Math.round(downloadProgress)}%</div>
          <button className="btn-secondary" onClick={handleCancel}>Cancel</button>
        </div>
      )}

      {/* Download errors */}
      {status.type === "error" && (
        <div className="error-banner">⚠ {(status as any).message}</div>
      )}

      {/* Model selection — always visible */}
      <div className="model-section">
        <label className="section-label">Model size</label>
        <div className="model-size-cards">
          {MODEL_SIZES.map((m) => (
            <button
              key={m.value}
              className={`size-card ${selectedSize === m.value ? "selected" : ""}`}
              onClick={() => setSelectedSize(m.value)}
              disabled={isDownloading}
            >
              <div className="size-card-title">{m.label}</div>
              <div className="size-card-desc">{m.description}</div>
              <div className="size-card-vram">{m.vram}</div>
            </button>
          ))}
        </div>
      </div>

      <div className="model-section">
        <label className="section-label">Quantization</label>
        <div className="quant-options">
          {QUANTIZATIONS.map((q) => (
            <label key={q.value} className="quant-option">
              <input
                type="radio"
                name="quant"
                value={q.value}
                checked={selectedQuant === q.value}
                onChange={() => setSelectedQuant(q.value)}
                disabled={isDownloading}
              />
              <span className="quant-label">{q.label}</span>
              <span className="quant-size">{q.size}</span>
            </label>
          ))}
        </div>
      </div>

      <div className="model-section">
        <label className="section-label">
          Install location{" "}
          <span className="section-hint">(leave blank for default)</span>
        </label>
        <input
          type="text"
          className="path-input"
          placeholder="%LOCALAPPDATA%\DeepM\models"
          value={modelPath}
          onChange={(e) => setModelPath(e.target.value)}
          disabled={isDownloading}
        />
      </div>

      {isReady ? (
        <div className="model-action-row">
          <span className="model-ready-badge">✓ Model loaded</span>
          <button className="btn-secondary" onClick={handleDownload}>
            Re-download / change model
          </button>
          <button className="btn-primary" onClick={handleRetryEngine} disabled={engineRetrying}>
            {engineRetrying ? "Restarting…" : "Restart engine"}
          </button>
        </div>
      ) : (
        <button
          className="btn-primary download-btn"
          onClick={handleDownload}
          disabled={isDownloading}
        >
          {isDownloading ? "Downloading…" : `Download ${selectedSize} (${selectedQuant})`}
        </button>
      )}
    </div>
  );
}
