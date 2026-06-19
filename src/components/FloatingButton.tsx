import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Copy, X, RotateCcw, Languages } from "lucide-react";
import "./FloatingButton.css";

type UIState = "idle" | "loading" | "result" | "error";

type FloatingShow = {
  text: string;
  source_lang: string;
  target_lang: string;
};

export default function FloatingButton() {
  const [uiState, setUiState] = useState<UIState>("idle");
  const [translation, setTranslation] = useState("");
  const [langLabel, setLangLabel] = useState("");
  const [visible, setVisible] = useState(false);

  const uiStateRef = useRef<UIState>("idle");
  useEffect(() => { uiStateRef.current = uiState; }, [uiState]);

  // The selection captured by the backend, ready to translate on click.
  const pendingTextRef = useRef("");
  const pendingSrcRef = useRef("auto");
  const pendingTgtRef = useRef("ru");

  // Force the document background fully transparent (overrides global.css).
  useEffect(() => {
    const t = "background:transparent!important;";
    document.documentElement.style.cssText += t;
    document.body.style.cssText += t;
    const root = document.getElementById("root");
    if (root) root.style.cssText += t;
  }, []);

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

  // Translate the already-captured selection. The backend grabbed the text when
  // the button appeared, so clicking translates instantly without copying again.
  const runTranslate = async () => {
    const txt = pendingTextRef.current;
    if (!txt.trim()) { doHide(); return; }
    setUiState("loading");
    try {
      const result = await invoke<string>("quick_translate", {
        sourceText: txt,
        sourceLang: pendingSrcRef.current,
        targetLang: pendingTgtRef.current,
      });
      setTranslation(result);
      setUiState("result");
      setExpanded(true);
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

    // Backend captured a selection: store it and show the (idle) button.
    const unsub = listen<FloatingShow>("floating_show", (e) => {
      const p = e.payload;
      pendingTextRef.current = p.text ?? "";
      pendingSrcRef.current = p.source_lang ?? "auto";
      pendingTgtRef.current = p.target_lang ?? "ru";
      setTranslation("");
      setLangLabel(`${(p.source_lang ?? "").toUpperCase()} → ${(p.target_lang ?? "").toUpperCase()}`);
      setUiState("idle");
      setExpanded(false);
      setVisible(true);
    });

    return () => {
      unsub.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const handleBtnClick = () => {
    const s = uiStateRef.current;
    if (s === "idle") {
      runTranslate();
    } else if (s === "loading") {
      // ignore — wait for result
    } else {
      doHide();
    }
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(translation);
    doHide();
  };

  const handleRetry = () => runTranslate();

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
          uiState === "idle"    ? "Перевести выделенное" :
          uiState === "loading" ? "Переводим…" : "Закрыть"
        }
      >
        <div className="fb-btn">
          {uiState === "loading"
            ? <span className="fb-spinner" />
            : <Languages size={18} strokeWidth={1.8} />}
        </div>
      </div>

      {/* Translation card — selectable; no preventDefault so the user can
          drag-select part of the translation and Ctrl+C it. */}
      {isExpanded && (
        <div className="fb-card-wrap">
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
