import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  return "ru";
}

export default function FloatingButton() {
  const [uiState, setUiState] = useState<UIState>("idle");
  const [translation, setTranslation] = useState("");
  const [langLabel, setLangLabel] = useState("");
  // visible controls whether the button is rendered at all (pointer-events etc)
  const [visible, setVisible] = useState(false);

  // Refs keep current values for use inside event listener callbacks (no stale closures)
  const pendingTextRef = useRef("");
  const pendingTgtRef  = useRef("ru");
  const uiStateRef     = useRef<UIState>("idle");

  // Keep ref in sync with state
  useEffect(() => { uiStateRef.current = uiState; }, [uiState]);

  // Force transparent background immediately — overrides global.css body rule.
  useEffect(() => {
    const t = "background:transparent!important;";
    document.documentElement.style.cssText += t;
    document.body.style.cssText += t;
    const root = document.getElementById("root");
    if (root) root.style.cssText += t;
  }, []);

  const doHide = () => {
    setVisible(false);
    setUiState("idle");
    setTranslation("");
    setLangLabel("");
    pendingTextRef.current = "";
    invoke("hide_floating_button").catch(() => {});
  };

  const runTranslate = async (txt: string, tgt: string) => {
    if (!txt.trim()) return;
    setUiState("loading");
    try {
      const result = await invoke<string>("quick_translate", {
        sourceText: txt,
        sourceLang: "auto",
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
    // Escape key dismisses the button
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") doHide();
    };
    window.addEventListener("keydown", onKey);

    // floating_text: new text arrived from selection
    const unsubText = listen<{ text: string }>("floating_text", async (e) => {
      const incoming = e.payload.text;
      pendingTextRef.current = incoming;
      pendingTgtRef.current  = "ru"; // default until detection finishes
      setTranslation("");
      setLangLabel("");
      setUiState("idle");
      setVisible(true);

      // Detect language in background; update label + ref when done
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

  const handleBtnClick = () => {
    const state = uiStateRef.current;
    if (state === "idle") {
      runTranslate(pendingTextRef.current, pendingTgtRef.current);
    } else if (state === "loading") {
      // wait — do nothing
    } else {
      // result or error: dismiss
      doHide();
    }
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(translation);
    doHide();
  };

  const handleRetry = () => {
    runTranslate(pendingTextRef.current, pendingTgtRef.current);
  };

  const isExpanded = uiState === "result" || uiState === "error";

  if (!visible) {
    // Render nothing when hidden — prevents transparent-window click-blocking
    return <div className="fb-root" />;
  }

  return (
    <div className="fb-root">
      {/* Circle translate button */}
      <div
        className={`fb-btn-wrap${uiState === "loading" ? " fb-btn-wrap-loading" : ""}`}
        onClick={handleBtnClick}
        title={
          uiState === "idle"    ? "Перевести" :
          uiState === "loading" ? "Переводим…" : "Закрыть"
        }
      >
        <div className="fb-btn">
          {uiState === "loading"
            ? <span className="fb-spinner" />
            : <Languages size={22} strokeWidth={1.8} />
          }
        </div>
      </div>

      {/* Translation card — slides in below button when expanded */}
      <div className={`fb-card-wrap${isExpanded ? " fb-card-visible" : ""}`}>
        <div className={`fb-card${uiState === "error" ? " fb-card-error" : ""}`}>
          <div className="fb-card-header">
            <span className="fb-lang-badge">{langLabel}</span>
            <div className="fb-card-actions">
              {uiState === "error" && (
                <button className="fb-icon-btn" onClick={handleRetry} title="Повторить">
                  <RotateCcw size={14} />
                </button>
              )}
              {uiState === "result" && (
                <button className="fb-icon-btn" onClick={handleCopy} title="Копировать">
                  <Copy size={14} />
                </button>
              )}
              <button className="fb-icon-btn fb-close-btn" onClick={doHide} title="Закрыть">
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
