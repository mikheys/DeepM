import React, { useEffect, useState } from "react";
import { Download, Trash2, Loader2 } from "lucide-react";
import { ocrLangsStatus, ocrLangDownload, ocrLangRemove } from "../api";
import type { OcrLang } from "../types";
import { useI18n } from "../i18n-context";
import "./OcrLanguages.css";

type Props = {
  enabled: string[];
  onToggle: (code: string, on: boolean) => void;
};

export default function OcrLanguages({ enabled, onToggle }: Props) {
  const { t } = useI18n();
  const [langs, setLangs] = useState<OcrLang[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = () => ocrLangsStatus().then(setLangs).catch(() => {});
  useEffect(() => { refresh(); }, []);

  const download = async (code: string) => {
    setBusy(code);
    try { await ocrLangDownload(code); } finally { setBusy(null); refresh(); }
  };
  const remove = async (code: string) => {
    setBusy(code);
    try { await ocrLangRemove(code); onToggle(code, false); } finally { setBusy(null); refresh(); }
  };

  return (
    <div className="ocr-langs">
      {langs.map((l) => (
        <div className="ocr-lang-row" key={l.code}>
          <label className="ocr-lang-name" title={!l.installed ? t.ocr_lang_install_first : ""}>
            <input
              type="checkbox"
              checked={enabled.includes(l.code)}
              disabled={!l.installed}
              onChange={(e) => onToggle(l.code, e.target.checked)}
            />
            <span>{l.name}</span>
            <span className="ocr-lang-code">{l.code}</span>
          </label>
          <div className="ocr-lang-actions">
            {busy === l.code ? (
              <span className="ocr-lang-busy"><Loader2 size={14} className="spin" /></span>
            ) : l.installed ? (
              l.bundled ? (
                <span className="ocr-lang-badge">{t.ocr_lang_bundled}</span>
              ) : (
                <button className="ocr-lang-btn danger" onClick={() => remove(l.code)} title={t.ocr_lang_remove}>
                  <Trash2 size={14} />
                </button>
              )
            ) : (
              <button className="ocr-lang-btn" onClick={() => download(l.code)} title={t.ocr_lang_download}>
                <Download size={14} /> {t.ocr_lang_download}
              </button>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
