import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getSettings } from "../api";
import { Copy, X, RotateCcw, Languages } from "lucide-react";
import "./FloatingButton.css";

type UIState = "idle" | "loading" | "result" | "error";

// Compact: just the square button.
// Expanded: button row + popup card.
const BTN_SIZE = 52;
const POPUP_W = 300;
const POPUP_H = 160;
const GAP = 6;

/** Detect the primary language from text (2-char code or "?") */
async function detectLang(text: string): Promise<string> {
  try {
    return await invoke<string>("detect_language", { text });
  } catch {
    return "en";
  }
}

/** Auto-pick target based on detected source: EN↔RU, otherwise English */
function autoTarget(detected: string): string {
  if (detected === "ru") return "en";
  if (detected === "en") return "ru";
  return "en";
}

export default function FloatingButton() {
  const [text, setText] = useState("");
  const [translation, setTranslation] = useState("");
  const [uiState, setUiState] = useState<UIState>("idle");
  const [sourceLang, setSourceLang] = useState("auto");
  const [targetLang, setTargetLang] = useState("en");
  const [detectedLang, setDetectedLang] = useState<string | null>(null);
  const defaultTargetRef = useRef("en");

  // Make the window background fully transparent so rounded corners work
  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    const root = document.getElementById("root");
    if (root) root.style.background = "transparent";
  }, []);

  useEffect(() => {
    getSettings().then((s) => {
      defaultTargetRef.current = s.default_target_lang;
      setTargetLang(s.default_target_lang);
      setSourceLang(s.default_source_lang);
    }).catch(() => {});

    const unsubText = listen<{ text: string }>("floating_text", async (e) => {
      const incoming = e.payload.text;
      setText(incoming);
      setTranslation("");
      setUiState("idle");

      // Auto-detect and choose opposite language
      const detected = await detectLang(incoming);
      setDetectedLang(detected);
      setTargetLang(autoTarget(detected));
      await getCurrentWindow().setSize(new LogicalSize(BTN_SIZE, BTN_SIZE)).catch(() => {});
    });

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") doHide();
    };
    window.addEventListener("keydown", onKey);

    return () => {
      unsubText.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  // Resize window when state changes
  useEffect(() => {
    const isExpanded = uiState === "result" || uiState === "error";
    if (isExpanded) {
      getCurrentWindow()
        .setSize(new LogicalSize(POPUP_W, BTN_SIZE + GAP + POPUP_H))
        .catch(() => {});
    } else {
      getCurrentWindow()
        .setSize(new LogicalSize(BTN_SIZE, BTN_SIZE))
        .catch(() => {});
    }
  }, [uiState]);

  const doHide = () => {
    invoke("hide_floating_button").catch(() => {});
    setUiState("idle");
    setText("");
    setTranslation("");
    setDetectedLang(null);
  };

  const doTranslate = async () => {
    if (!text.trim()) return;
    setUiState("loading");
    try {
      const result = await invoke<string>("quick_translate", {
        sourceText: text,
        sourceLang,
        targetLang,
      });
      setTranslation(result);
      setUiState("result");
    } catch (e) {
      setTranslation(String(e));
      setUiState("error");
    }
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(translation);
    doHide();
  };

  const isExpanded = uiState === "result" || uiState === "error";
  const langLabel = detectedLang
    ? `${detectedLang.toUpperCase()} → ${targetLang.toUpperCase()}`
    : `→ ${targetLang.toUpperCase()}`;

  return (
    <div className={`fb-root ${isExpanded ? "fb-expanded" : ""}`}>
      {/* Square icon button — always visible */}
      <button
        className={`fb-btn ${uiState === "loading" ? "fb-btn-loading" : ""}`}
        onClick={uiState === "idle" ? doTranslate : undefined}
        title={uiState === "idle" ? "Translate" : undefined}
      >
        {uiState === "loading" ? (
          <span className="fb-spinner" />
        ) : (
          <Languages size={22} strokeWidth={1.8} />
        )}
      </button>

      {/* Popup card — shown only when expanded */}
      {isExpanded && (
        <div className={`fb-card ${uiState === "error" ? "fb-card-error" : ""}`}>
          <div className="fb-card-header">
            <span className="fb-lang-badge">{langLabel}</span>
            <div className="fb-card-actions">
              {uiState === "error" && (
                <button className="fb-icon-action" onClick={doTranslate} title="Retry">
                  <RotateCcw size={14} />
                </button>
              )}
              {uiState === "result" && (
                <button className="fb-icon-action" onClick={handleCopy} title="Copy">
                  <Copy size={14} />
                </button>
              )}
              <button className="fb-icon-action fb-close" onClick={doHide} title="Close">
                <X size={14} />
              </button>
            </div>
          </div>
          <div className="fb-card-body">
            <p className={uiState === "error" ? "fb-text-error" : "fb-text-result"}>
              {translation}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
