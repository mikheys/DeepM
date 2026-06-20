import React, { useState, useEffect } from "react";
import type { AppSettings, GlossaryEntry } from "../types";
import type { Locale } from "../i18n";
import { LANGUAGES, TARGET_LANGUAGES } from "../types";
import { getSettings, saveSettings, gpuStatus } from "../api";
import { useI18n } from "../i18n-context";
import HotkeyCapture from "./HotkeyCapture";
import ExclusionsModal from "./ExclusionsModal";
import "./SettingsPanel.css";

type Props = {
  onClose?: () => void;
  locale: Locale;
  onLocaleChange: (l: Locale) => void;
};

export default function SettingsPanel({ onClose, locale, onLocaleChange }: Props) {
  const { t } = useI18n();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);
  const [newGlossarySource, setNewGlossarySource] = useState("");
  const [newGlossaryTarget, setNewGlossaryTarget] = useState("");
  const [newGlossaryPair, setNewGlossaryPair] = useState("en->ru");
  const [showExclusions, setShowExclusions] = useState(false);
  const [cudaReady, setCudaReady] = useState(true);
  const [nvidiaPresent, setNvidiaPresent] = useState(true);

  useEffect(() => {
    getSettings().then((s) => {
      // Reflect real GPU capability: force GPU off when it can't actually run.
      gpuStatus().then((g) => {
        setCudaReady(g.cuda_ready);
        setNvidiaPresent(g.nvidia_present);
        setSettings(g.cuda_ready ? s : { ...s, use_gpu: false });
      }).catch(() => setSettings(s));
    }).catch(() => {});
  }, []);

  const gpuHint = cudaReady
    ? t.settings_gpu_hint
    : nvidiaPresent
      ? t.settings_gpu_unavailable     // NVIDIA present, pack missing
      : t.settings_gpu_no_nvidia;      // no NVIDIA GPU at all

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    setSettings((prev) => prev ? { ...prev, [key]: value } : prev);
    setSaved(false);
  };

  const updateHotkey = (key: keyof AppSettings["hotkeys"], value: string) => {
    setSettings((prev) =>
      prev ? { ...prev, hotkeys: { ...prev.hotkeys, [key]: value } } : prev
    );
    setSaved(false);
  };

  const addGlossaryEntry = () => {
    if (!newGlossarySource.trim() || !newGlossaryTarget.trim()) return;
    const entry: GlossaryEntry = {
      id: crypto.randomUUID(),
      source: newGlossarySource.trim(),
      target: newGlossaryTarget.trim(),
      lang_pair: newGlossaryPair,
    };
    update("glossary", [...(settings?.glossary ?? []), entry]);
    setNewGlossarySource("");
    setNewGlossaryTarget("");
  };

  const removeGlossaryEntry = (id: string) => {
    update("glossary", (settings?.glossary ?? []).filter((e) => e.id !== id));
  };

  const handleSave = async () => {
    if (!settings) return;
    const toSave = { ...settings, locale };
    await saveSettings(toSave);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  if (!settings) return <div className="settings-loading">{t.loading}</div>;

  return (
    <div className="settings-panel">
      <div className="settings-content">
        <section className="settings-section">
          <h2 className="settings-section-title">{t.settings_translation}</h2>
          <div className="settings-row">
            <label>{t.settings_default_source}</label>
            <select
              value={settings.default_source_lang}
              onChange={(e) => update("default_source_lang", e.target.value)}
            >
              {LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>{l.name}</option>
              ))}
            </select>
          </div>
          <div className="settings-row">
            <label>{t.settings_default_target}</label>
            <select
              value={settings.default_target_lang}
              onChange={(e) => update("default_target_lang", e.target.value)}
            >
              {TARGET_LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>{l.name}</option>
              ))}
            </select>
          </div>
          <div className="settings-row">
            <label>{t.settings_gpu}</label>
            <input
              type="checkbox"
              checked={cudaReady && settings.use_gpu}
              disabled={!cudaReady}
              onChange={(e) => update("use_gpu", e.target.checked)}
            />
            <span className="settings-hint">{gpuHint}</span>
          </div>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">{t.settings_glossary}</h2>
          <p className="settings-desc">{t.settings_glossary_desc}</p>
          <div className="glossary-add-row">
            <input
              type="text"
              placeholder={t.settings_source_term}
              value={newGlossarySource}
              onChange={(e) => setNewGlossarySource(e.target.value)}
              className="glossary-input"
            />
            <span className="glossary-arrow">→</span>
            <input
              type="text"
              placeholder={t.settings_translation_term}
              value={newGlossaryTarget}
              onChange={(e) => setNewGlossaryTarget(e.target.value)}
              className="glossary-input"
            />
            <select
              value={newGlossaryPair}
              onChange={(e) => setNewGlossaryPair(e.target.value)}
              className="glossary-pair-select"
            >
              <option value="en->ru">EN→RU</option>
              <option value="ru->en">RU→EN</option>
              <option value="en->zh">EN→ZH</option>
              <option value="zh->en">ZH→EN</option>
            </select>
            <button className="btn-add" onClick={addGlossaryEntry}>{t.settings_add}</button>
          </div>
          {settings.glossary.length > 0 && (
            <div className="glossary-list">
              {settings.glossary.map((e) => (
                <div key={e.id} className="glossary-entry">
                  <span className="glossary-pair-tag">{e.lang_pair}</span>
                  <span className="glossary-source">{e.source}</span>
                  <span className="glossary-arrow-small">→</span>
                  <span className="glossary-target">{e.target}</span>
                  <button
                    className="glossary-remove"
                    onClick={() => removeGlossaryEntry(e.id)}
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">{t.settings_hotkeys}</h2>

          <div className="settings-row">
            <label>{t.settings_translate_replace}</label>
            <HotkeyCapture
              value={settings.hotkeys.translate_replace}
              onChange={(v) => updateHotkey("translate_replace", v)}
            />
            <span className="settings-hint">{t.settings_hotkey_hint}</span>
          </div>

          <div className="settings-row">
            <label>{t.settings_copy_taps}</label>
            <select
              value={settings.triple_copy_count}
              onChange={(e) => {
                const n = Number(e.target.value);
                update("triple_copy_count", n);
                // keep the display string in sync (e.g. "Ctrl+C ×2")
                updateHotkey("triple_copy", `Ctrl+C ×${n}`);
              }}
            >
              <option value={2}>{t.taps_double}</option>
              <option value={3}>{t.taps_triple}</option>
            </select>
          </div>

          <div className="settings-row">
            <label>{t.settings_triple_interval}</label>
            <input
              type="number"
              className="hotkey-input"
              value={settings.triple_copy_interval_ms}
              min={200}
              max={2000}
              onChange={(e) => update("triple_copy_interval_ms", Number(e.target.value))}
            />
          </div>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">{t.settings_interface}</h2>
          <div className="settings-row">
            <label>{t.settings_floating}</label>
            <input
              type="checkbox"
              checked={settings.show_floating_button}
              onChange={(e) => update("show_floating_button", e.target.checked)}
            />
            <span className="settings-hint">{t.settings_floating_hint}</span>
          </div>
          <div className="settings-row">
            <label>{t.settings_exclusions}</label>
            <button className="btn-secondary" onClick={() => setShowExclusions(true)}>
              {t.settings_exclusions_btn}
              {settings.floating_exclusions.length > 0
                ? ` (${settings.floating_exclusions.length})`
                : ""}
            </button>
            <span className="settings-hint">{t.settings_exclusions_hint}</span>
          </div>
          <div className="settings-row">
            <label>{t.settings_ocr_engine}</label>
            <select
              value={settings.ocr_engine}
              onChange={(e) => update("ocr_engine", e.target.value)}
            >
              <option value="windows">{t.ocr_engine_windows}</option>
              <option value="tesseract">{t.ocr_engine_tesseract}</option>
              <option value="rapidocr">{t.ocr_engine_rapidocr}</option>
            </select>
            <span className="settings-hint">{t.settings_ocr_engine_hint}</span>
          </div>
          <div className="settings-row">
            <label>{t.settings_autostart}</label>
            <input
              type="checkbox"
              checked={settings.autostart}
              onChange={(e) => update("autostart", e.target.checked)}
            />
          </div>
          <div className="settings-row">
            <label>{t.settings_start_tray}</label>
            <input
              type="checkbox"
              checked={settings.start_in_tray}
              onChange={(e) => update("start_in_tray", e.target.checked)}
            />
          </div>
          <div className="settings-row">
            <label>{t.settings_locale}</label>
            <select
              value={locale}
              onChange={(e) => onLocaleChange(e.target.value as Locale)}
            >
              <option value="en">English</option>
              <option value="ru">Русский</option>
            </select>
          </div>
        </section>
      </div>

      <div className="settings-footer">
        {saved && <span className="saved-indicator">{t.settings_saved}</span>}
        <button className="btn-primary" onClick={handleSave}>
          {t.settings_save}
        </button>
      </div>

      {showExclusions && (
        <ExclusionsModal
          value={settings.floating_exclusions}
          onChange={(next) => update("floating_exclusions", next)}
          onClose={() => setShowExclusions(false)}
        />
      )}
    </div>
  );
}
