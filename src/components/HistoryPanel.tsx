import React, { useState, useEffect } from "react";
import type { TranslationHistoryEntry } from "../types";
import { getHistory, clearHistory } from "../api";
import { LANGUAGES } from "../types";
import { useI18n } from "../i18n-context";
import { Copy, ClipboardCheck, RefreshCw } from "lucide-react";
import "./HistoryPanel.css";

type Props = {
  onSelect?: (entry: TranslationHistoryEntry) => void;
};

export default function HistoryPanel({ onSelect }: Props) {
  const { t } = useI18n();
  const [entries, setEntries] = useState<TranslationHistoryEntry[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    // Server inserts newest at index 0, so the list is already newest-first.
    const h = await getHistory().catch(() => []);
    setEntries(h);
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const handleClear = async () => {
    if (!confirm(t.confirm_clear_history)) return;
    await clearHistory();
    setEntries([]);
  };

  const langName = (code: string) =>
    LANGUAGES.find((l) => l.code === code)?.nativeName ?? code;

  const copy = (key: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey((k) => (k === key ? null : k)), 1200);
  };

  // Toggle expand on click — unless the user is selecting text.
  const toggle = (id: string) => {
    if ((window.getSelection()?.toString() ?? "").length > 0) return;
    setExpandedId((cur) => (cur === id ? null : id));
  };

  const filtered = query
    ? entries.filter(
        (e) =>
          e.source_text.toLowerCase().includes(query.toLowerCase()) ||
          e.translated_text.toLowerCase().includes(query.toLowerCase())
      )
    : entries;

  return (
    <div className="history-panel">
      <div className="history-toolbar">
        <input
          type="search"
          className="search-input"
          placeholder={t.search_placeholder}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {entries.length > 0 && (
          <button className="btn-danger-sm" onClick={handleClear}>
            {t.clear_all}
          </button>
        )}
      </div>

      {loading ? (
        <div className="history-empty">{t.loading}</div>
      ) : filtered.length === 0 ? (
        <div className="history-empty">
          {query ? t.no_matches : t.no_history}
        </div>
      ) : (
        <div className="history-list">
          {filtered.map((entry) => {
            const open = expandedId === entry.id;
            return (
              <div
                key={entry.id}
                className={`history-entry${open ? " history-entry-open" : ""}`}
                onClick={() => toggle(entry.id)}
              >
                <div className="entry-langs">
                  <span>{langName(entry.source_lang)} → {langName(entry.target_lang)}</span>
                  <span className="entry-time">
                    {new Date(entry.timestamp).toLocaleString()}
                  </span>
                </div>
                <div className="entry-source">{entry.source_text}</div>
                <div className="entry-translation">{entry.translated_text}</div>

                {open && (
                  <div className="entry-actions" onClick={(e) => e.stopPropagation()}>
                    <button className="entry-action"
                      onClick={() => copy(`${entry.id}-s`, entry.source_text)}>
                      {copiedKey === `${entry.id}-s`
                        ? <ClipboardCheck size={13} /> : <Copy size={13} />}
                      {t.history_copy_source}
                    </button>
                    <button className="entry-action"
                      onClick={() => copy(`${entry.id}-t`, entry.translated_text)}>
                      {copiedKey === `${entry.id}-t`
                        ? <ClipboardCheck size={13} /> : <Copy size={13} />}
                      {t.history_copy_result}
                    </button>
                    <button className="entry-action entry-action-accent"
                      onClick={() => onSelect?.(entry)}>
                      <RefreshCw size={13} />
                      {t.history_retranslate}
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
