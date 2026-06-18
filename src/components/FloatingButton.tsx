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

  // Refs hold the current text/lang so the click handler never has a stale closure.
  const pendingTextRef = useRef("");
  const pendingSrcRef  = useRef("auto");
  const pendingTgtRef  = useRef("en");

  // Force transparent background immediately — overrides global.css body rule.
  useEffect(() => {
    const t = "background:transparent!important;";
    document.documentElement.style.cssText += t;
    document.body.style.cssText += t;
    const root = document.getElementById("root");
    if (root) root.style.cssText += t;
  }, []);

  useEffect(() => {
    getSettings().catch(() => {});
  }, []);

  // Translate using values from refs — stale-closure safe.
  const translateDirectly = async (txt: string, src: string, tgt: string) => {
    if (!txt.trim()) return;
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

    // When text arrives: store it in refs for the click handler.
    // Do NOT auto-translate — user must click the button.
    const unsubText = listen<{ text: string }>("floating_text", async (e) => {
      const incoming = e.payload.text;

      // Store text immediately so click handler has it right away.
      pendingTextRef.current = incoming;
      pendingSrcRef.current  = "auto";

      setTranslation("");
      setUiState("idle");

      // Detect target lang in background; update refs and label when ready.
      const detected = await detectLang(incoming);
      const tgt = autoTarget(detected);
      pendingTgtRef.current = tgt;
      setLangLabel(`${detected.toUpperCase()} → ${tgt.toUpperCase()}`);
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
    pendingTextRef.current = "";
  };

  // Click logic:
  //   idle → translate (the whole point of the button)
  //   loading → ignore (wait for result)
  //   result / error → dismiss
  const handleBtnClick = () => {
    if (uiState === "idle") {
      translateDirectly(pendingTextRef.current, pendingSrcRef.current, pendingTgtRef.current);
    } else if (uiState === "loading") {
      // do nothing — let the translation finish
    } else {
      doHide();
    }
  };

  const doRetry = () => {
    translateDirectly(pendingTextRef.current, pendingSrcRef.current, pendingTgtRef.current);
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(translation);
    doHide();
  };

  const isExpanded = uiState === "result" || uiState === "error";

  return (
    <div className="fb-root">
      {/* Button — 52×52, always visible at top of window */}
      <div
        className={`fb-btn-wrap ${uiState === "loading" ? "fb-btn-wrap-loading" : ""}`}
        onClick={handleBtnClick}
        title={uiState === "idle" ? "Translate" : uiState === "loading" ? "Translating…" : "Close"}
      >
        <button className="fb-btn" tabIndex={-1}>
          {uiState === "loading" ? (
            <span className="fb-spinner" />
          ) : (
            <Languages size={22} strokeWidth={1.8} />
          )}
        </button>
      </div>

      {/* Popup card — appears below button when expanded */}
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
