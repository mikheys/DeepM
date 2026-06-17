import React, { useState, useEffect } from "react";
import type { TranslationHistoryEntry } from "../types";
import { getHistory, clearHistory } from "../api";
import { LANGUAGES } from "../types";
import "./HistoryPanel.css";

type Props = {
  onSelect?: (entry: TranslationHistoryEntry) => void;
};

export default function HistoryPanel({ onSelect }: Props) {
  const [entries, setEntries] = useState<TranslationHistoryEntry[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);

  const load = async () => {
    setLoading(true);
    const h = await getHistory().catch(() => []);
    setEntries(h.reverse());
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const handleClear = async () => {
    if (!confirm("Clear all translation history?")) return;
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
          placeholder="Search history…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {entries.length > 0 && (
          <button className="btn-danger-sm" onClick={handleClear}>
            Clear all
          </button>
        )}
      </div>

      {loading ? (
        <div className="history-empty">Loading…</div>
      ) : filtered.length === 0 ? (
        <div className="history-empty">
          {query ? "No matches found." : "No translation history yet."}
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
