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
  return detected === "ru" ? "en" : "ru";
}

export default function FloatingButton() {
  const [uiState, setUiState] = useState<UIState>("idle");
  const [translation, setTranslation] = useState("");
  const [langLabel, setLangLabel] = useState("");
  const [visible, setVisible] = useState(false);

  // Refs hold current values for use inside event-listener callbacks (no stale closures).
  const pendingTextRef = useRef("");
  const pendingTgtRef  = useRef("ru");
  const uiStateRef     = useRef<UIState>("idle");

  useEffect(() => { uiStateRef.current = uiState; }, [uiState]);

  // Force the document background fully transparent (overrides global.css).
  useEffect(() => {
    const t = "background:transparent!important;";
    document.documentElement.style.cssText += t;
    document.body.style.cssText += t;
    const root = document.getElementById("root");
    if (root) root.style.cssText += t;
  }, []);

  // Tell the Rust side to grow/shrink the OS window whenever we expand/collapse.
  const setExpanded = (expanded: boolean) => {
    invoke("set_floating_expanded", { expanded }).catch(() => {});
  };

  const doHide = () => {
    setExpanded(false);
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
      setExpanded(true); // grow the window to reveal the card
    } catch (e) {
      setTranslation(String(e));
      setUiState("error");
      setExpanded(true);
    }
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") doHide();
    };
    window.addEventListener("keydown", onKey);

    // New selection arrived: show the collapsed button, detect language in background.
    const unsubText = listen<{ text: string }>("floating_text", async (e) => {
      const incoming = e.payload.text;
      pendingTextRef.current = incoming;
      pendingTgtRef.current  = "ru";
      setTranslation("");
      setLangLabel("");
      setUiState("idle");
      setExpanded(false);
      setVisible(true);

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
      // ignore — wait for result
    } else {
      doHide();
    }
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(translation);
    doHide();
  };

  const handleRetry = () => runTranslate(pendingTextRef.current, pendingTgtRef.current);

  // Prevent the button/card from taking focus away from the source app.
  const noFocus = (e: React.MouseEvent) => e.preventDefault();

  const isExpanded = uiState === "result" || uiState === "error";

  if (!visible) return <div className="fb-root" />;

  return (
    <div className="fb-root">
      {/* Round translate button, anchored top-left */}
      <div
        className={`fb-btn-wrap${uiState === "loading" ? " fb-btn-wrap-loading" : ""}`}
        onMouseDown={noFocus}
        onClick={handleBtnClick}
        title={
          uiState === "idle"    ? "Перевести" :
          uiState === "loading" ? "Переводим…" : "Закрыть"
        }
      >
        <div className="fb-btn">
          {uiState === "loading"
            ? <span className="fb-spinner" />
            : <Languages size={18} strokeWidth={1.8} />}
        </div>
      </div>

      {/* Translation card */}
      {isExpanded && (
        <div className="fb-card-wrap" onMouseDown={noFocus}>
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
      )}
    </div>
  );
}
