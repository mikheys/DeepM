import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getSettings } from "../api";
import "./FloatingButton.css";

type UIState = "idle" | "loading" | "result" | "error";

const COMPACT = 52;
const WIDE = 320;
const HEIGHT = 52;

export default function FloatingButton() {
  const [text, setText] = useState("");
  const [translation, setTranslation] = useState("");
  const [uiState, setUiState] = useState<UIState>("idle");
  const [targetLang, setTargetLang] = useState("en");
  const [sourceLang, setSourceLang] = useState("auto");

  useEffect(() => {
    getSettings().then((s) => {
      setTargetLang(s.default_target_lang);
      setSourceLang(s.default_source_lang);
    }).catch(() => {});

    const unsubText = listen<{ text: string }>("floating_text", (e) => {
      setText(e.payload.text);
      setTranslation("");
      setUiState("idle"); // show the button; user clicks to translate
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
    const isWide = uiState === "result" || uiState === "error";
    getCurrentWindow()
      .setSize(new LogicalSize(isWide ? WIDE : COMPACT, HEIGHT))
      .catch(() => {});
  }, [uiState]);

  const doHide = () => {
    invoke("hide_floating_button").catch(() => {});
    getCurrentWindow().setSize(new LogicalSize(COMPACT, HEIGHT)).catch(() => {});
    setUiState("idle");
    setText("");
    setTranslation("");
  };

  const doTranslate = async (src: string) => {
    setUiState("loading");
    try {
      const result = await invoke<string>("quick_translate", {
        sourceText: src,
        sourceLang,
        targetLang,
      });
      setTranslation(result);
      setUiState("result");
    } catch (e) {
      setTranslation(`${e}`);
      setUiState("error");
    }
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(translation);
    doHide();
  };

  return (
    <div className={`fb-root fb-${uiState}`}>
      {uiState === "idle" && (
        <button
          className="fb-icon-btn"
          onClick={() => text.trim() && doTranslate(text)}
          title="Translate selection"
        >
          ⇄
        </button>
      )}

      {uiState === "loading" && (
        <div className="fb-loading">
          <span className="fb-spinner" />
        </div>
      )}

      {(uiState === "result" || uiState === "error") && (
        <div className="fb-pill">
          <span className={uiState === "error" ? "fb-error-text" : "fb-result-text"}>
            {translation}
          </span>
          {uiState === "result" && (
            <button className="fb-action-btn" onClick={handleCopy} title="Copy">
              Copy
            </button>
          )}
          {uiState === "error" && (
            <button className="fb-action-btn" onClick={() => doTranslate(text)} title="Retry">
              ↺
            </button>
          )}
          <button className="fb-action-btn fb-close-btn" onClick={doHide} title="Close">
            ✕
          </button>
        </div>
      )}
    </div>
  );
}
