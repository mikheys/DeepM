import React, { useState, useEffect, useCallback } from "react";
import type { ModelSize, Quantization } from "../types";
import {
  getModelStatus,
  startModelDownload,
  cancelModelDownload,
  restartEngine,
  loadModel,
  loadExternalModel,
  getDownloadState,
  listDownloadedModels,
  deleteModel,
  onDownloadProgress,
  onModelReady,
  onModelError,
  onModelDownloaded,
  onDownloadCancelled,
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

const key = (size: string, quant: string) => `${size}/${quant}`;

// Derive the active built-in variant key from a model file path (null if it's
// an external/custom model).
function variantFromPath(path: string): string | null {
  const m = path.match(/HY-MT1\.5-(1\.8B|7B)-(Q4_K_M|Q6_K|Q8_0)\.gguf$/i);
  return m ? key(m[1], m[2]) : null;
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export default function ModelManager({ onModelReady: onReady, isOnboarding }: Props) {
  const { t } = useI18n();
  const [activeVariant, setActiveVariant] = useState<string | null>(null);
  const [activePath, setActivePath] = useState<string | null>(null);
  const [isExternalActive, setIsExternalActive] = useState(false);

  const [downloaded, setDownloaded] = useState<Set<string>>(new Set());
  const [downloadingKey, setDownloadingKey] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadSpeed, setDownloadSpeed] = useState(0);

  const [busyKey, setBusyKey] = useState<string | null>(null); // variant being loaded
  const [deletingKey, setDeletingKey] = useState<string | null>(null);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [engineBusy, setEngineBusy] = useState(false);
  const [externalPath, setExternalPath] = useState("");

  const refreshDownloaded = useCallback(async () => {
    try {
      const pairs = await listDownloadedModels();
      setDownloaded(new Set(pairs.map(([s, q]) => key(s, q))));
    } catch {
      setDownloaded(new Set());
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await getModelStatus();
      if (s.type === "ready") {
        const path = (s as any).path as string;
        const v = variantFromPath(path ?? "");
        setActiveVariant(v);
        setActivePath(path ?? null);
        setIsExternalActive(!!path && !v);
      } else {
        setActiveVariant(null);
        setActivePath(null);
        setIsExternalActive(false);
      }
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    refreshStatus();
    refreshDownloaded();

    // Resume an in-progress download if we're re-entering the tab.
    getDownloadState().then((d) => {
      if (d) {
        setDownloadingKey(key(d.size, d.quantization));
        setDownloadProgress(d.progress);
        setDownloadSpeed(d.speed_mbps);
      }
    }).catch(() => {});

    const subs = [
      onDownloadProgress((p, speed) => { setDownloadProgress(p); setDownloadSpeed(speed); }),
      onModelReady(() => {
        setEngineBusy(false);
        setBusyKey(null);
        setEngineError(null);
        setDownloadingKey(null);
        refreshStatus();
        refreshDownloaded();
        onReady();
      }),
      onModelError((msg) => {
        setEngineBusy(false);
        setBusyKey(null);
        setEngineError(msg);
      }),
      onModelDownloaded(() => {
        setDownloadingKey(null);
        refreshDownloaded();
      }),
      onDownloadCancelled(() => {
        setDownloadingKey(null);
        setDownloadProgress(0);
      }),
    ];

    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, [refreshStatus, refreshDownloaded, onReady]);

  const handleDownload = async (size: ModelSize, quant: Quantization) => {
    const k = key(size, quant);
    setDownloadingKey(k);
    setDownloadProgress(0);
    setEngineError(null);
    try {
      await startModelDownload(size, quant);
    } catch (e) {
      setEngineError(String(e));
      setDownloadingKey(null);
    }
  };

  const handleCancel = async () => {
    await cancelModelDownload().catch(() => {});
    setDownloadingKey(null);
    setDownloadProgress(0);
  };

  const handleLoad = async (size: ModelSize, quant: Quantization) => {
    const k = key(size, quant);
    setBusyKey(k);
    setEngineError(null);
    try {
      await loadModel(size, quant); // fire-and-forget; model_ready/error finishes it
    } catch (e) {
      setEngineError(String(e));
      setBusyKey(null);
    }
  };

  const handleDelete = async (size: ModelSize, quant: Quantization) => {
    const k = key(size, quant);
    setDeletingKey(k);
    try {
      await deleteModel(size, quant);
      await refreshDownloaded();
      if (activeVariant === k) { setActiveVariant(null); refreshStatus(); }
    } catch (e) {
      setEngineError(String(e));
    } finally {
      setDeletingKey(null);
    }
  };

  const handleReloadEngine = () => {
    setEngineBusy(true);
    setEngineError(null);
    restartEngine().catch((e) => { setEngineError(String(e)); setEngineBusy(false); });
  };

  const handleLoadExternal = () => {
    if (!externalPath.trim()) return;
    setEngineBusy(true);
    setEngineError(null);
    loadExternalModel(externalPath.trim()).catch((e) => {
      setEngineError(String(e));
      setEngineBusy(false);
    });
  };

  const isDownloadingAny = downloadingKey !== null;
  const anyBusy = isDownloadingAny || busyKey !== null || engineBusy;

  return (
    <div className={`model-manager ${isOnboarding ? "onboarding-mode" : ""}`}>
      {isOnboarding && (
        <div className="onboarding-header">
          <h1 className="onboarding-title">{t.onboarding_title}</h1>
          <p className="onboarding-subtitle">{t.onboarding_subtitle}</p>
        </div>
      )}

      {/* Active model */}
      {(activeVariant || isExternalActive) && (
        <div className="model-ready-card">
          <span className="ready-icon">✓</span>
          <div>
            <div className="ready-title">
              {t.model_active}{" "}
              {isExternalActive ? baseName(activePath ?? "") : activeVariant?.replace("/", " ")}
            </div>
            {!isOnboarding && (
              <button
                className="btn-secondary engine-retry-btn"
                onClick={handleReloadEngine}
                disabled={engineBusy}
                style={{ marginTop: 6 }}
              >
                {engineBusy ? t.restarting : t.restart_engine}
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
        </div>
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

      {/* Built-in variants */}
      <div className="model-section">
        <label className="section-label">
          {t.available_variants}
          <span className="section-hint">{t.variants_hint}</span>
        </label>
        <div className="variant-grid">
          {VARIANTS.map((v) => {
            const k = key(v.size, v.quant);
            const isActive = activeVariant === k;
            const isPresent = downloaded.has(k);
            const isThisDownloading = downloadingKey === k;
            const isThisLoading = busyKey === k;
            const isThisDeleting = deletingKey === k;
            return (
              <div key={k} className={`variant-row${isActive ? " variant-active" : ""}`}>
                <div className="variant-info">
                  <span className="variant-label">{v.label}</span>
                  <span className="variant-size">{v.fileSize}</span>
                </div>
                <div className="variant-actions">
                  {isThisDownloading ? (
                    <span className="variant-downloading">{Math.round(downloadProgress)}%</span>
                  ) : isPresent ? (
                    <>
                      {isActive ? (
                        <span className="variant-loaded-badge">{t.loaded_badge}</span>
                      ) : (
                        <button
                          className="btn-small btn-load"
                          disabled={anyBusy}
                          onClick={() => handleLoad(v.size, v.quant)}
                        >
                          {isThisLoading ? "…" : t.load_btn}
                        </button>
                      )}
                      <button
                        className="btn-small btn-delete"
                        disabled={isThisDeleting || anyBusy || isActive}
                        onClick={() => handleDelete(v.size, v.quant)}
                        title={isActive ? t.cannot_delete_active : t.delete_btn}
                      >
                        {isThisDeleting ? "…" : t.delete_btn}
                      </button>
                    </>
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

      {/* External model */}
      <div className="model-section">
        <label className="section-label">
          {t.model_external_title}
          <span className="section-hint">{t.model_external_hint}</span>
        </label>
        <div className="external-row">
          <input
            type="text"
            className="external-input"
            placeholder={t.model_external_placeholder}
            value={externalPath}
            onChange={(e) => setExternalPath(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") handleLoadExternal(); }}
          />
          <button
            className="btn-small btn-load"
            disabled={!externalPath.trim() || anyBusy}
            onClick={handleLoadExternal}
          >
            {t.load_btn}
          </button>
        </div>
      </div>
    </div>
  );
}
