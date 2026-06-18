import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getSettings } from "../api";
import { Copy, X, RotateCcw, Languages } from "lucide-react";
import "./FloatingButton.css";

type UIState = "idle" | "loading" | "result" | "error";

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
  // Refs hold the latest translation params for retry (avoid stale closures)
  const lastTextRef = useRef("");
  const lastSrcRef = useRef("auto");
  const lastTgtRef = useRef("en");

  // Force the whole window transparent immediately — overrides global.css body background
  useEffect(() => {
    const force = "background:transparent!important;";
    document.documentElement.style.cssText += force;
    document.body.style.cssText += force;
    const root = document.getElementById("root");
    if (root) root.style.cssText += force;
  }, []);

  useEffect(() => {
    getSettings().catch(() => {});
  }, []);

  // Translate directly — all params passed in, no React state needed (stale-closure safe)
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
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") doHide();
    };
    window.addEventListener("keydown", onKey);

    // floating_text → detect → auto-translate (no click needed)
    const unsubText = listen<{ text: string }>("floating_text", async (e) => {
      const incoming = e.payload.text;
      setTranslation("");
      setUiState("idle");

      const detected = await detectLang(incoming);
      const tgt = autoTarget(detected);
      setLangLabel(`${detected.toUpperCase()} → ${tgt.toUpperCase()}`);
      await translateDirectly(incoming, "auto", tgt);
    });

    return () => {
      unsubText.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

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
    <div className="fb-root">
      {/* Button row — always at top, 52×52, transparent around it */}
      <div className="fb-btn-wrap" onClick={doHide} title="Close">
        <button
          className={`fb-btn ${uiState === "loading" ? "fb-btn-loading" : ""}`}
          tabIndex={-1}
        >
          {uiState === "loading" ? (
            <span className="fb-spinner" />
          ) : (
            <Languages size={22} strokeWidth={1.8} />
          )}
        </button>
      </div>

      {/* Popup card — appears below when expanded */}
      <div className={`fb-card-wrap ${isExpanded ? "fb-card-visible" : ""}`}>
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
      </div>
    </div>
  );
}
