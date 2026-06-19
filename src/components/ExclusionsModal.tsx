import React, { useState, useEffect } from "react";
import { X, Plus, RefreshCw, FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { listAppProcesses } from "../api";
import { useI18n } from "../i18n-context";
import "./ExclusionsModal.css";

type Props = {
  value: string[];
  onChange: (next: string[]) => void;
  onClose: () => void;
};

function normalize(name: string): string {
  // Match on the executable's base name (that's what the backend reports).
  let n = (name.split(/[\\/]/).pop() ?? name).trim().toLowerCase();
  if (!n) return "";
  if (!n.endsWith(".exe")) n += ".exe";
  return n;
}

export default function ExclusionsModal({ value, onChange, onClose }: Props) {
  const { t } = useI18n();
  const [running, setRunning] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [manual, setManual] = useState("");

  const loadRunning = async () => {
    setLoading(true);
    try {
      setRunning(await listAppProcesses());
    } catch {
      setRunning([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { loadRunning(); }, []);

  const add = (name: string) => {
    const n = normalize(name);
    if (!n || value.some((v) => v.toLowerCase() === n)) return;
    onChange([...value, n]);
  };

  const remove = (name: string) => {
    onChange(value.filter((v) => v.toLowerCase() !== name.toLowerCase()));
  };

  const addManual = () => {
    add(manual);
    setManual("");
  };

  const browse = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: "Executable", extensions: ["exe"] }],
    }).catch(() => null);
    if (file && typeof file === "string") add(file);
  };

  // Running apps not already excluded
  const available = running.filter(
    (r) => !value.some((v) => v.toLowerCase() === r.toLowerCase())
  );

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>{t.exclusions_title}</h3>
          <button className="modal-close" onClick={onClose} title={t.exclusions_close}>
            <X size={16} />
          </button>
        </div>

        <p className="modal-desc">{t.exclusions_desc}</p>

        {/* Current exclusions */}
        <div className="excl-section-label">{t.exclusions_current}</div>
        {value.length === 0 ? (
          <div className="excl-empty">{t.exclusions_empty}</div>
        ) : (
          <div className="excl-chips">
            {value.map((v) => (
              <span key={v} className="excl-chip">
                {v}
                <button onClick={() => remove(v)} title={t.exclusions_remove}>
                  <X size={12} />
                </button>
              </span>
            ))}
          </div>
        )}

        {/* Add an app */}
        <div className="excl-section-label">{t.exclusions_manual}</div>
        <button className="excl-browse" onClick={browse}>
          <FolderOpen size={14} /> {t.exclusions_browse}
        </button>
        <div className="excl-manual-row">
          <input
            type="text"
            placeholder="example.exe"
            value={manual}
            onChange={(e) => setManual(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") addManual(); }}
          />
          <button className="btn-add" onClick={addManual} disabled={!manual.trim()}>
            <Plus size={14} /> {t.exclusions_add}
          </button>
        </div>

        {/* Running apps */}
        <div className="excl-section-label excl-running-label">
          {t.exclusions_running}
          <button className="excl-refresh" onClick={loadRunning} title={t.exclusions_refresh}>
            <RefreshCw size={13} />
          </button>
        </div>
        {loading ? (
          <div className="excl-empty">{t.loading}</div>
        ) : available.length === 0 ? (
          <div className="excl-empty">{t.exclusions_none_running}</div>
        ) : (
          <div className="excl-running-list">
            {available.map((r) => (
              <button key={r} className="excl-running-item" onClick={() => add(r)}>
                <Plus size={13} /> {r}
              </button>
            ))}
          </div>
        )}

        <div className="modal-footer">
          <button className="btn-primary" onClick={onClose}>{t.exclusions_done}</button>
        </div>
      </div>
    </div>
  );
}
