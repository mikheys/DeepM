import React, { useState, useEffect } from "react";
import type { AppSettings, GlossaryEntry } from "../types";
import { LANGUAGES, TARGET_LANGUAGES } from "../types";
import { getSettings, saveSettings } from "../api";
import "./SettingsPanel.css";

type Props = {
  onClose?: () => void;
};

export default function SettingsPanel({ onClose }: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);
  const [newGlossarySource, setNewGlossarySource] = useState("");
  const [newGlossaryTarget, setNewGlossaryTarget] = useState("");
  const [newGlossaryPair, setNewGlossaryPair] = useState("en->zh");

  useEffect(() => {
    getSettings().then(setSettings).catch(() => {});
  }, []);

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
    await saveSettings(settings);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  if (!settings) return <div className="settings-loading">Loading settings…</div>;

  return (
    <div className="settings-panel">
      <div className="settings-content">
        <section className="settings-section">
          <h2 className="settings-section-title">Translation</h2>
          <div className="settings-row">
            <label>Default source language</label>
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
            <label>Default target language</label>
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
            <label>GPU acceleration</label>
            <input
              type="checkbox"
              checked={settings.use_gpu}
              onChange={(e) => update("use_gpu", e.target.checked)}
            />
            <span className="settings-hint">Use CUDA if available</span>
          </div>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">Model</h2>
          <div className="settings-row">
            <label>Model size</label>
            <select
              value={settings.model_size}
              onChange={(e) => update("model_size", e.target.value as any)}
            >
              <option value="1.8B">HY-MT1.5-1.8B</option>
              <option value="7B">HY-MT1.5-7B</option>
            </select>
          </div>
          <div className="settings-row">
            <label>Quantization</label>
            <select
              value={settings.quantization}
              onChange={(e) => update("quantization", e.target.value as any)}
            >
              <option value="Q4_K_M">Q4_K_M (recommended)</option>
              <option value="Q6_K">Q6_K</option>
              <option value="Q8_0">Q8_0</option>
            </select>
          </div>
        </section>

        <section className="settings-section">
          <h2 className="settings-section-title">Glossary</h2>
          <p className="settings-desc">
            Terms here are passed to the model via terminology intervention.
          </p>
          <div className="glossary-add-row">
            <input
              type="text"
              placeholder="Source term"
              value={newGlossarySource}
              onChange={(e) => setNewGlossarySource(e.target.value)}
              className="glossary-input"
            />
            <span className="glossary-arrow">→</span>
            <input
              type="text"
              placeholder="Translation"
              value={newGlossaryTarget}
              onChange={(e) => setNewGlossaryTarget(e.target.value)}
              className="glossary-input"
            />
            <select
              value={newGlossaryPair}
              onChange={(e) => setNewGlossaryPair(e.target.value)}
              className="glossary-pair-select"
            >
              <option value="en->zh">EN→ZH</option>
              <option value="zh->en">ZH→EN</option>
              <option value="en->ru">EN→RU</option>
              <option value="ru->en">RU→EN</option>
            </select>
            <button className="btn-add" onClick={addGlossaryEntry}>Add</button>
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
          <h2 className="settings-section-title">Hotkeys</h2>
          <p className="settings-desc settings-note">
            Hotkey customization will be active in Stage 2. These values are saved and will be used.
          </p>
          <div className="settings-row">
            <label>Triple-copy trigger</label>
            <input
              type="text"
              className="hotkey-input"
              value={settings.hotkeys.triple_copy}
              onChange={(e) => updateHotkey("triple_copy", e.target.value)}
            />
          </div>
          <div className="settings-row">
            <label>Translate &amp; replace</label>
            <input
              type="text"
              className="hotkey-input"
              value={settings.hotkeys.translate_replace}
              onChange={(e) => updateHotkey("translate_replace", e.target.value)}
            />
          </div>
          <div className="settings-row">
            <label>Triple-copy interval (ms)</label>
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
          <h2 className="settings-section-title">Interface</h2>
          <div className="settings-row">
            <label>Show floating button</label>
            <input
              type="checkbox"
              checked={settings.show_floating_button}
              onChange={(e) => update("show_floating_button", e.target.checked)}
            />
            <span className="settings-hint">Appears when text is selected</span>
          </div>
          <div className="settings-row">
            <label>Start with Windows</label>
            <input
              type="checkbox"
              checked={settings.autostart}
              onChange={(e) => update("autostart", e.target.checked)}
            />
          </div>
          <div className="settings-row">
            <label>Start in tray</label>
            <input
              type="checkbox"
              checked={settings.start_in_tray}
              onChange={(e) => update("start_in_tray", e.target.checked)}
            />
          </div>
        </section>
      </div>

      <div className="settings-footer">
        {saved && <span className="saved-indicator">Saved ✓</span>}
        <button className="btn-primary" onClick={handleSave}>
          Save settings
        </button>
      </div>
    </div>
  );
}
