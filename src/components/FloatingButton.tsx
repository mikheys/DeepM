import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getSettings } from "../api";
import { Copy, X, RotateCcw, Languages } from "lucide-react";
import "./FloatingButton.css";

type UIState = "idle" | "loading" | "result" | "error";

const BTN_SIZE = 52;
const POPUP_W = 300;
const POPUP_H = 160;
const GAP = 6;

async function detectLang(text: string): Promise<string> {
  try {
    return await invoke<string>("detect_language", { text });
  } catch {
    return "en";
  }
}

function autoTarget(detected: string): string {
  if (detected === "ru") return "en";
  if (detected === "en") return "ru";
  return "en";
}

export default function FloatingButton() {
  const [translation, setTranslation] = useState("");
  const [uiState, setUiState] = useState<UIState>("idle");
  const [langLabel, setLangLabel] = useState("");
  const defaultTargetRef = useRef("en");
  // Store current translation params in refs to allow retry without stale closures
  const lastTextRef = useRef("");
  const lastSrcRef = useRef("auto");
  const lastTgtRef = useRef("en");

  // Force transparent background immediately — runs before any paint
  useEffect(() => {
    document.documentElement.style.cssText += ";background:transparent!important";
    document.body.style.cssText += ";background:transparent!important";
    const root = document.getElementById("root");
    if (root) root.style.cssText += ";background:transparent!important";
  }, []);

  // Load default settings once
  useEffect(() => {
    getSettings().then((s) => {
      defaultTargetRef.current = s.default_target_lang;
    }).catch(() => {});
  }, []);

  // Translate directly without depending on React state (avoids stale closures)
  const translateDirectly = async (txt: string, src: string, tgt: string) => {
    if (!txt.trim()) return;
    lastTextRef.current = txt;
    lastSrcRef.current = src;
    lastTgtRef.current = tgt;
    setUiState("loading");
    try {
      const result = await invoke<string>("quick_translate", {
        sourceText: txt,
        sourceLang: src,
        targetLang: tgt,
      });
      setTranslation(result);
      setUiState("result");
    } catch (e) {
      setTranslation(String(e));
      setUiState("error");
    }
  };

  useEffect(() => {
    // Escape key hides the window
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") doHide();
    };
    window.addEventListener("keydown", onKey);

    // floating_text → detect language → auto-translate immediately (no click needed)
    const unsubText = listen<{ text: string }>("floating_text", async (e) => {
      const incoming = e.payload.text;
      setTranslation("");
      setUiState("idle");

      // Detect language, then translate immediately
      const detected = await detectLang(incoming);
      const tgt = autoTarget(detected);
      const label = `${detected.toUpperCase()} → ${tgt.toUpperCase()}`;
      setLangLabel(label);

      // Auto-translate: passes values directly to avoid stale-closure bug
      await translateDirectly(incoming, "auto", tgt);
    });

    return () => {
      unsubText.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  // Resize window when ui state changes
  useEffect(() => {
    const expanded = uiState === "result" || uiState === "error";
    getCurrentWindow()
      .setSize(new LogicalSize(
        expanded ? POPUP_W : BTN_SIZE,
        expanded ? BTN_SIZE + GAP + POPUP_H : BTN_SIZE,
      ))
      .catch(() => {});
  }, [uiState]);

  const doHide = () => {
    invoke("hide_floating_button").catch(() => {});
    setUiState("idle");
    setTranslation("");
    setLangLabel("");
  };

  const doRetry = () => {
    translateDirectly(lastTextRef.current, lastSrcRef.current, lastTgtRef.current);
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(translation);
    doHide();
  };

  const isExpanded = uiState === "result" || uiState === "error";

  return (
    <div className={`fb-root ${isExpanded ? "fb-expanded" : ""}`}>
      {/* Square button — click always dismisses */}
      <button
        className={`fb-btn ${uiState === "loading" ? "fb-btn-loading" : ""}`}
        onClick={doHide}
        title="Close"
      >
        {uiState === "loading" ? (
          <span className="fb-spinner" />
        ) : (
          <Languages size={22} strokeWidth={1.8} />
        )}
      </button>

      {isExpanded && (
        <div className={`fb-card ${uiState === "error" ? "fb-card-error" : ""}`}>
          <div className="fb-card-header">
            <span className="fb-lang-badge">{langLabel}</span>
            <div className="fb-card-actions">
              {uiState === "error" && (
                <button className="fb-icon-action" onClick={doRetry} title="Retry">
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
