import React, { useState, useEffect } from "react";
import type { TranslationHistoryEntry } from "../types";
import { getHistory, clearHistory } from "../api";
import { LANGUAGES } from "../types";
import { useI18n } from "../i18n-context";
import "./HistoryPanel.css";

type Props = {
  onSelect?: (entry: TranslationHistoryEntry) => void;
};

export default function HistoryPanel({ onSelect }: Props) {
  const { t } = useI18n();
  const [entries, setEntries] = useState<TranslationHistoryEntry[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);

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
          {filtered.map((entry) => (
            <button
              key={entry.id}
              className="history-entry"
              onClick={() => onSelect?.(entry)}
            >
              <div className="entry-langs">
                {langName(entry.source_lang)} → {langName(entry.target_lang)}
                <span className="entry-time">
                  {new Date(entry.timestamp).toLocaleString()}
                </span>
              </div>
              <div className="entry-source">{entry.source_text}</div>
              <div className="entry-translation">{entry.translated_text}</div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
