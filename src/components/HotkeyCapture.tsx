import React, { useState } from "react";
import { useI18n } from "../i18n-context";

type Props = {
  value: string;
  onChange: (combo: string) => void;
};

// Maps a KeyboardEvent.code to the canonical key name the Rust matcher expects.
function codeToName(code: string): string | null {
  const m1 = /^Key([A-Z])$/.exec(code);
  if (m1) return m1[1];
  const m2 = /^Digit([0-9])$/.exec(code);
  if (m2) return m2[1];
  if (/^F([1-9]|1[0-2])$/.test(code)) return code;
  return null;
}

/**
 * A click-to-record hotkey field. Click it, then press a combination such as
 * Ctrl+Shift+T — it records the modifiers + main key. At least one modifier is
 * required so the global shortcut doesn't collide with normal typing.
 */
export default function HotkeyCapture({ value, onChange }: Props) {
  const { t } = useI18n();
  const [capturing, setCapturing] = useState(false);

  const onKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      setCapturing(false);
      return;
    }

    const name = codeToName(e.code);
    if (!name) return; // a modifier alone — keep waiting for the main key

    const parts: string[] = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.shiftKey) parts.push("Shift");
    if (e.altKey) parts.push("Alt");
    if (parts.length === 0) return; // require at least one modifier

    parts.push(name);
    onChange(parts.join("+"));
    setCapturing(false);
  };

  return (
    <button
      type="button"
      className={`hotkey-capture${capturing ? " capturing" : ""}`}
      onClick={() => setCapturing(true)}
      onBlur={() => setCapturing(false)}
      onKeyDown={capturing ? onKeyDown : undefined}
      title={t.settings_hotkey_hint}
    >
      {capturing ? t.hotkey_press : value}
    </button>
  );
}
