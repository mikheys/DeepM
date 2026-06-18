import React, { useState, useEffect, useCallback } from "react";
import type { ModelSize, Quantization, ModelStatus } from "../types";
import {
  getModelStatus,
  startModelDownload,
  cancelModelDownload,
  restartEngine,
  listDownloadedModels,
  deleteModel,
  onDownloadProgress,
  onModelReady,
  onModelError,
} from "../api";
import { useI18n } from "../i18n-context";
import "./ModelManager.css";

type Props = {
  onModelReady: () => void;
  isOnboarding?: boolean;
};

type Variant = { size: ModelSize; quant: Quantization; label: string; fileSize: string };

const VARIANTS: Variant[] = [
  { size: "1.8B", quant: "Q4_K_M", label: "1.8B Q4_K_M", fileSize: "~1.1 GB" },
  { size: "1.8B", quant: "Q6_K",   label: "1.8B Q6_K",   fileSize: "~1.5 GB" },
  { size: "1.8B", quant: "Q8_0",   label: "1.8B Q8_0",   fileSize: "~1.9 GB" },
  { size: "7B",   quant: "Q4_K_M", label: "7B Q4_K_M",   fileSize: "~4.4 GB" },
  { size: "7B",   quant: "Q6_K",   label: "7B Q6_K",     fileSize: "~5.7 GB" },
  { size: "7B",   quant: "Q8_0",   label: "7B Q8_0",     fileSize: "~7.7 GB" },
];

function variantKey(size: string, quant: string) { return `${size}/${quant}`; }

export default function ModelManager({ onModelReady: onReady, isOnboarding }: Props) {
  const { t } = useI18n();
  const [status, setStatus] = useState<ModelStatus>({ type: "not_downloaded" });
  const [downloaded, setDownloaded] = useState<Set<string>>(new Set());
  const [activeVariant, setActiveVariant] = useState<string | null>(null);
  const [downloadingKey, setDownloadingKey] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadSpeed, setDownloadSpeed] = useState(0);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [engineRetrying, setEngineRetrying] = useState(false);
  const [deletingKey, setDeletingKey] = useState<string | null>(null);

  const refreshDownloaded = useCallback(async () => {
    try {
      const pairs = await listDownloadedModels();
      setDownloaded(new Set(pairs.map(([s, q]) => variantKey(s, q))));
    } catch {
      // model dir may not exist yet
      setDownloaded(new Set());
    }
  }, []);

  useEffect(() => {
    getModelStatus().then((s) => {
      setStatus(s);
      if (s.type === "ready") {
        const path = (s as any).path as string;
        const match = path.match(/HY-MT1\.5-(\w+\.?\w*)-(\w+)\.gguf$/i);
        if (match) setActiveVariant(variantKey(match[1], match[2]));
      }
    }).catch(() => {});

    refreshDownloaded();

    const unsubProgress = onDownloadProgress((p, speed) => {
      setDownloadProgress(p);
      setDownloadSpeed(speed);
    });
    const unsubReady = onModelReady(() => {
      setStatus({ type: "ready", path: "" });
      setEngineError(null);
      setDownloadingKey(null);
      refreshDownloaded();
      onReady();
    });
    const unsubError = onModelError((msg) => {
      if (msg.includes("llama-server") || msg.includes("crashed") || msg.includes("start")) {
        setEngineError(msg);
      } else {
        setStatus({ type: "error", message: msg });
        setDownloadingKey(null);
      }
    });

    return () => {
      unsubProgress.then((f) => f());
      unsubReady.then((f) => f());
      unsubError.then((f) => f());
    };
  }, []);

  const handleDownload = async (size: ModelSize, quant: Quantization) => {
    const key = variantKey(size, quant);
    setDownloadingKey(key);
    setDownloadProgress(0);
    setEngineError(null);
    try {
      await startModelDownload(size, quant);
    } catch (e) {
      setStatus({ type: "error", message: String(e) });
      setDownloadingKey(null);
    }
  };

  const handleCancel = async () => {
    await cancelModelDownload();
    setDownloadingKey(null);
    setDownloadProgress(0);
    setStatus({ type: "not_downloaded" });
  };

  const handleDelete = async (size: ModelSize, quant: Quantization) => {
    const key = variantKey(size, quant);
    setDeletingKey(key);
    try {
      await deleteModel(size, quant);
      await refreshDownloaded();
      if (activeVariant === key) {
        setActiveVariant(null);
        setStatus({ type: "not_downloaded" });
      }
    } catch (e) {
      setEngineError(String(e));
    } finally {
      setDeletingKey(null);
    }
  };

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

  const isReady = status.type === "ready";
  const isDownloadingAny = downloadingKey !== null;

  return (
    <div className={`model-manager ${isOnboarding ? "onboarding-mode" : ""}`}>
      {isOnboarding && (
        <div className="onboarding-header">
          <h1 className="onboarding-title">{t.onboarding_title}</h1>
          <p className="onboarding-subtitle">{t.onboarding_subtitle}</p>
        </div>
      )}

      {/* Active model badge */}
      {isReady && activeVariant && (
        <div className="model-ready-card">
          <span className="ready-icon">✓</span>
          <div>
            <div className="ready-title">{t.model_active} {activeVariant.replace("/", " ")}</div>
            {!isOnboarding && (
              <button
                className="btn-secondary engine-retry-btn"
                onClick={handleRetryEngine}
                disabled={engineRetrying}
                style={{ marginTop: 6 }}
              >
                {engineRetrying ? t.restarting : t.restart_engine}
              </button>
            )}
          </div>
        </div>
      )}

      {/* Engine error */}
      {engineError && (
        <div className="engine-error-card">
          <div className="engine-error-title">⚠ {t.engine_error}</div>
          <div className="engine-error-detail">{engineError}</div>
          <button
            className="btn-primary engine-retry-btn"
            onClick={handleRetryEngine}
            disabled={engineRetrying}
          >
            {engineRetrying ? t.starting : t.retry}
          </button>
        </div>
      )}

      {/* Download error */}
      {status.type === "error" && (
        <div className="error-banner">⚠ {(status as any).message}</div>
      )}

      {/* Download progress */}
      {isDownloadingAny && (
        <div className="download-progress-card">
          <div className="progress-label">
            <span>{t.downloading} {downloadingKey?.replace("/", " ")}…</span>
            <span className="progress-speed">{downloadSpeed.toFixed(1)} MB/s</span>
          </div>
          <div className="progress-bar-track">
            <div className="progress-bar-fill" style={{ width: `${downloadProgress}%` }} />
          </div>
          <div className="progress-pct">{Math.round(downloadProgress)}%</div>
          <button className="btn-secondary" onClick={handleCancel}>{t.cancel}</button>
        </div>
      )}

      {/* Variant grid */}
      <div className="model-section">
        <label className="section-label">
          {t.available_variants}
          <span className="section-hint">{t.variants_hint}</span>
        </label>
        <div className="variant-grid">
          {VARIANTS.map((v) => {
            const key = variantKey(v.size, v.quant);
            const isActive = activeVariant === key;
            const isPresent = downloaded.has(key);
            const isThisDownloading = downloadingKey === key;
            const isThisDeleting = deletingKey === key;
            return (
              <div
                key={key}
                className={`variant-row${isActive ? " variant-active" : ""}`}
              >
                <div className="variant-info">
                  <span className="variant-label">{v.label}</span>
                  <span className="variant-size">{v.fileSize}</span>
                </div>
                <div className="variant-actions">
                  {isPresent ? (
                    <>
                      {isActive ? (
                        <span className="variant-loaded-badge">{t.loaded_badge}</span>
                      ) : (
                        <button
                          className="btn-small btn-load"
                          disabled={isDownloadingAny || engineRetrying}
                          onClick={() => {
                            setActiveVariant(key);
                            restartEngine().catch(() => {});
                          }}
                        >
                          {t.load_btn}
                        </button>
                      )}
                      <button
                        className="btn-small btn-delete"
                        disabled={isThisDeleting || isDownloadingAny || isActive}
                        onClick={() => handleDelete(v.size, v.quant)}
                        title={isActive ? t.cannot_delete_active : "Delete from disk"}
                      >
                        {isThisDeleting ? "…" : t.delete_btn}
                      </button>
                    </>
                  ) : isThisDownloading ? (
                    <span className="variant-downloading">
                      {Math.round(downloadProgress)}%
                    </span>
                  ) : (
                    <button
                      className="btn-small btn-download"
                      disabled={isDownloadingAny}
                      onClick={() => handleDownload(v.size, v.quant)}
                    >
                      {t.download_btn}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
