import React, { useState, useEffect, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { RotateCcw, Plus } from "lucide-react";
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

type Variant = {
  version: string;
  size: ModelSize;
  quant: Quantization;
  label: string;
  fileSize: string;
};
type ExternalModel = { path: string; name: string };

type Family = { version: string; title: string; note?: string };
const FAMILIES: Family[] = [
  { version: "Hy-MT2", title: "Hy-MT2", note: "newer" },
  { version: "HY-MT1.5", title: "HY-MT1.5" },
];

function variantsFor(version: string): Variant[] {
  return [
    { version, size: "1.8B", quant: "Q4_K_M", label: "1.8B Q4_K_M", fileSize: "~1.1 GB" },
    { version, size: "1.8B", quant: "Q6_K",   label: "1.8B Q6_K",   fileSize: "~1.5 GB" },
    { version, size: "1.8B", quant: "Q8_0",   label: "1.8B Q8_0",   fileSize: "~1.9 GB" },
    { version, size: "7B",   quant: "Q4_K_M", label: "7B Q4_K_M",   fileSize: "~4.6 GB" },
    { version, size: "7B",   quant: "Q6_K",   label: "7B Q6_K",     fileSize: "~6.2 GB" },
    { version, size: "7B",   quant: "Q8_0",   label: "7B Q8_0",     fileSize: "~8.0 GB" },
  ];
}

const key = (version: string, size: string, quant: string) => `${version}/${size}/${quant}`;
const EXTERNALS_KEY = "externalModels";

function variantFromPath(path: string): string | null {
  const f = path.toLowerCase();
  if (!f.endsWith(".gguf")) return null;
  const version = f.includes("mt2") ? "Hy-MT2" : f.includes("mt1.5") ? "HY-MT1.5" : null;
  const size = f.includes("1.8b") ? "1.8B" : f.includes("7b") ? "7B" : null;
  const quant = f.includes("q4_k_m") ? "Q4_K_M" : f.includes("q6_k") ? "Q6_K" : f.includes("q8_0") ? "Q8_0" : null;
  if (!version || !size || !quant) return null;
  return key(version, size, quant);
}
function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}
function loadExternals(): ExternalModel[] {
  try { return JSON.parse(localStorage.getItem(EXTERNALS_KEY) || "[]"); } catch { return []; }
}

export default function ModelManager({ onModelReady: onReady, isOnboarding }: Props) {
  const { t } = useI18n();
  const [activeVariant, setActiveVariant] = useState<string | null>(null);
  const [activePath, setActivePath] = useState<string | null>(null);

  const [downloaded, setDownloaded] = useState<Set<string>>(new Set());
  const [downloadingKey, setDownloadingKey] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadSpeed, setDownloadSpeed] = useState(0);

  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [deletingKey, setDeletingKey] = useState<string | null>(null);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [engineBusy, setEngineBusy] = useState(false);
  const [externals, setExternals] = useState<ExternalModel[]>(loadExternals);

  const saveExternals = (list: ExternalModel[]) => {
    setExternals(list);
    localStorage.setItem(EXTERNALS_KEY, JSON.stringify(list));
  };

  const refreshDownloaded = useCallback(async () => {
    try {
      const triples = await listDownloadedModels();
      setDownloaded(new Set(triples.map(([v, s, q]) => key(v, s, q))));
    } catch { setDownloaded(new Set()); }
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await getModelStatus();
      if (s.type === "ready") {
        const path = ((s as any).path as string) ?? "";
        setActiveVariant(variantFromPath(path));
        setActivePath(path || null);
      } else {
        setActiveVariant(null);
        setActivePath(null);
      }
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    refreshStatus();
    refreshDownloaded();
    getDownloadState().then((d) => {
      if (d) {
        setDownloadingKey(key(d.version, d.size, d.quantization));
        setDownloadProgress(d.progress);
        setDownloadSpeed(d.speed_mbps);
      }
    }).catch(() => {});

    const subs = [
      onDownloadProgress((p, speed) => { setDownloadProgress(p); setDownloadSpeed(speed); }),
      onModelReady(() => {
        setEngineBusy(false); setBusyKey(null); setEngineError(null); setDownloadingKey(null);
        refreshStatus(); refreshDownloaded(); onReady();
      }),
      onModelError((msg) => { setEngineBusy(false); setBusyKey(null); setEngineError(msg); }),
      onModelDownloaded(() => { setDownloadingKey(null); refreshDownloaded(); }),
      onDownloadCancelled(() => { setDownloadingKey(null); setDownloadProgress(0); }),
    ];
    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, [refreshStatus, refreshDownloaded, onReady]);

  const isDownloadingAny = downloadingKey !== null;
  const anyBusy = isDownloadingAny || busyKey !== null || engineBusy;
  const isExternalActive = !!activePath && !activeVariant;

  const handleDownload = async (v: string, size: ModelSize, quant: Quantization) => {
    setDownloadingKey(key(v, size, quant)); setDownloadProgress(0); setEngineError(null);
    try { await startModelDownload(v, size, quant); }
    catch (e) { setEngineError(String(e)); setDownloadingKey(null); }
  };

  const handleCancel = async () => {
    await cancelModelDownload().catch(() => {});
    setDownloadingKey(null); setDownloadProgress(0);
  };

  const handleLoad = async (v: string, size: ModelSize, quant: Quantization) => {
    setBusyKey(key(v, size, quant)); setEngineError(null);
    try { await loadModel(v, size, quant); }
    catch (e) { setEngineError(String(e)); setBusyKey(null); }
  };

  const handleDelete = async (v: string, size: ModelSize, quant: Quantization) => {
    const k = key(v, size, quant);
    setDeletingKey(k);
    try {
      await deleteModel(v, size, quant);
      await refreshDownloaded();
      if (activeVariant === k) { setActiveVariant(null); refreshStatus(); }
    } catch (e) { setEngineError(String(e)); }
    finally { setDeletingKey(null); }
  };

  const handleReloadEngine = () => {
    setEngineBusy(true); setEngineError(null);
    restartEngine().catch((e) => { setEngineError(String(e)); setEngineBusy(false); });
  };

  const handleAddExternal = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: "GGUF model", extensions: ["gguf"] }],
    }).catch(() => null);
    if (!file || typeof file !== "string") return;
    if (!externals.some((e) => e.path === file)) {
      saveExternals([...externals, { path: file, name: baseName(file) }]);
    }
    setEngineBusy(true); setEngineError(null);
    loadExternalModel(file).catch((e) => { setEngineError(String(e)); setEngineBusy(false); });
  };

  const handleLoadExternal = (path: string) => {
    setEngineBusy(true); setEngineError(null);
    loadExternalModel(path).catch((e) => { setEngineError(String(e)); setEngineBusy(false); });
  };

  const handleRemoveExternal = (path: string) => {
    saveExternals(externals.filter((e) => e.path !== path));
  };

  const hasActive = !!activeVariant || isExternalActive;

  const renderVariant = (v: Variant) => {
    const k = key(v.version, v.size, v.quant);
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
                <span className="variant-loaded-badge">✓ {t.loaded_badge}</span>
              ) : (
                <button className="btn-small btn-load" disabled={anyBusy}
                  onClick={() => handleLoad(v.version, v.size, v.quant)}>
                  {isThisLoading ? "…" : t.load_btn}
                </button>
              )}
              <button className="btn-small btn-delete"
                disabled={isThisDeleting || anyBusy || isActive}
                onClick={() => handleDelete(v.version, v.size, v.quant)}
                title={isActive ? t.cannot_delete_active : t.delete_btn}>
                {isThisDeleting ? "…" : t.delete_btn}
              </button>
            </>
          ) : (
            <button className="btn-small btn-download" disabled={isDownloadingAny}
              onClick={() => handleDownload(v.version, v.size, v.quant)}>
              {t.download_btn}
            </button>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className={`model-manager ${isOnboarding ? "onboarding-mode" : ""}`}>
      {isOnboarding ? (
        <div className="onboarding-header">
          <h1 className="onboarding-title">{t.onboarding_title}</h1>
          <p className="onboarding-subtitle">{t.onboarding_subtitle}</p>
        </div>
      ) : (
        <div className="mm-header">
          <span className="mm-title">{t.nav_model}</span>
          {hasActive && (
            <button className="mm-reload" onClick={handleReloadEngine}
              disabled={engineBusy} title={t.restart_engine}>
              <RotateCcw size={14} className={engineBusy ? "mm-reload-spin" : ""} />
              <span>{engineBusy ? t.restarting : t.restart_engine}</span>
            </button>
          )}
        </div>
      )}

      {engineError && (
        <div className="engine-error-card">
          <div className="engine-error-title">⚠ {t.engine_error}</div>
          <div className="engine-error-detail">{engineError}</div>
        </div>
      )}

      {isDownloadingAny && (
        <div className="download-progress-card">
          <div className="progress-label">
            <span>{t.downloading} {downloadingKey?.replace(/\//g, " ")}…</span>
            <span className="progress-speed">{downloadSpeed.toFixed(1)} MB/s</span>
          </div>
          <div className="progress-bar-track">
            <div className="progress-bar-fill" style={{ width: `${downloadProgress}%` }} />
          </div>
          <div className="progress-pct">{Math.round(downloadProgress)}%</div>
          <button className="btn-secondary" onClick={handleCancel}>{t.cancel}</button>
        </div>
      )}

      {/* Built-in model families */}
      {FAMILIES.map((fam) => (
        <div className="model-section" key={fam.version}>
          <label className="section-label">
            {fam.title}
            {fam.note === "newer" && <span className="model-new-tag">{t.model_newer_tag}</span>}
            <span className="section-hint">
              {fam.version === "Hy-MT2" ? t.model_v2_hint : t.variants_hint}
            </span>
          </label>
          <div className="variant-grid">
            {variantsFor(fam.version).map(renderVariant)}
          </div>
        </div>
      ))}

      {/* External models */}
      <div className="model-section">
        <label className="section-label">
          {t.model_external_title}
          <span className="section-hint">{t.model_external_hint}</span>
        </label>
        <div className="variant-grid">
          {externals.map((ext) => {
            const isActive = isExternalActive && activePath === ext.path;
            return (
              <div key={ext.path} className={`variant-row${isActive ? " variant-active" : ""}`}>
                <div className="variant-info">
                  <span className="variant-label">
                    {ext.name} <span className="external-tag">{t.model_external_tag}</span>
                  </span>
                  <span className="variant-size" title={ext.path}>{ext.path}</span>
                </div>
                <div className="variant-actions">
                  {isActive ? (
                    <span className="variant-loaded-badge">✓ {t.loaded_badge}</span>
                  ) : (
                    <button className="btn-small btn-load" disabled={anyBusy}
                      onClick={() => handleLoadExternal(ext.path)}>
                      {t.load_btn}
                    </button>
                  )}
                  <button className="btn-small btn-delete" disabled={isActive}
                    onClick={() => handleRemoveExternal(ext.path)}
                    title={isActive ? t.cannot_delete_active : t.delete_btn}>
                    {t.delete_btn}
                  </button>
                </div>
              </div>
            );
          })}
          <button className="add-model-btn" onClick={handleAddExternal} disabled={anyBusy}>
            <Plus size={15} /> {t.model_add_external}
          </button>
        </div>
      </div>
    </div>
  );
}
