import React, { useEffect, useState } from "react";
import { RefreshCw, Check, Download, ExternalLink, Bug } from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n-context";
import "./AboutPanel.css";

const REPO = "https://github.com/mikheys/DeepM";
const RELEASES_API = "https://api.github.com/repos/mikheys/DeepM/releases/latest";

/** Numeric semver-ish compare: returns 1 if a>b, -1 if a<b, 0 if equal. */
function cmpVersion(a: string, b: string): number {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] || 0;
    const y = pb[i] || 0;
    if (x > y) return 1;
    if (x < y) return -1;
  }
  return 0;
}

type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "latest" }
  | { kind: "available"; tag: string }
  | { kind: "error" };

export default function AboutPanel() {
  const { t } = useI18n();
  const [version, setVersion] = useState("");
  const [update, setUpdate] = useState<UpdateState>({ kind: "idle" });

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  const checkUpdates = async () => {
    setUpdate({ kind: "checking" });
    try {
      const res = await fetch(RELEASES_API, { headers: { Accept: "application/vnd.github+json" } });
      const data = await res.json();
      const tag = String(data.tag_name || "").replace(/^v/i, "");
      if (!tag) throw new Error("no tag");
      setUpdate(cmpVersion(tag, version) > 0 ? { kind: "available", tag } : { kind: "latest" });
    } catch {
      setUpdate({ kind: "error" });
    }
  };

  const ext = (url: string) => invoke("open_url", { url }).catch(() => {});

  return (
    <div className="about-panel">
      <div className="about-card">
        <svg className="about-logo" viewBox="0 0 512 512" width="72" height="72" aria-hidden>
          <rect width="512" height="512" rx="100" fill="#1b2236" />
          <g transform="translate(256 256) scale(0.72) translate(-256 -256)">
            <path
              fill="#fbfbfb"
              d="M396.87,0h61.29c5.38,0,9.75,4.37,9.75,9.75v321.98h-79.58V124.01l-80.46,96.17-42.53,50.83c-4.87,5.83-13.82,5.83-18.68,0l-42.53-50.83-80.48-96.18v207.74H44.09V9.75c0-5.38,4.37-9.75,9.75-9.75h61.29c5.41,0,10.54,2.4,14.01,6.55l125,149.39c.97,1.17,2.76,1.17,3.73,0L382.86,6.55c3.47-4.14,8.6-6.55,14.01-6.55h0Z"
            />
            <path
              fill="#5d6cb2"
              d="M467.9,331.74v22.7c0,40.35-32.71,73.07-73.07,73.07h-91.88c-2.89,0-5.69,1.03-7.89,2.91l-92.47,78.66c-7.9,6.72-20.07,1.11-20.07-9.28v-66.2c0-3.36-2.73-6.09-6.09-6.09h-59.27c-40.35,0-73.07-32.71-73.07-73.07v-22.7h79.56v8.91c0,10.77,8.73,19.48,19.48,19.48h225.71c10.77,0,19.48-8.73,19.48-19.48v-8.91h79.58Z"
            />
          </g>
        </svg>

        <h1 className="about-name">DeepM</h1>
        <div className="about-version">{version ? `v${version}` : ""}</div>
        <p className="about-tagline">{t.about_tagline}</p>

        <div className="about-update">
          <button className="btn-primary" onClick={checkUpdates} disabled={update.kind === "checking"}>
            <RefreshCw size={15} className={update.kind === "checking" ? "spin" : ""} />
            {t.about_check_updates}
          </button>
          {update.kind === "latest" && (
            <span className="about-update-msg ok"><Check size={15} /> {t.about_latest}</span>
          )}
          {update.kind === "available" && (
            <button className="btn-secondary about-update-msg" onClick={() => ext(`${REPO}/releases/latest`)}>
              <Download size={15} /> {t.about_available} v{update.tag}
            </button>
          )}
          {update.kind === "error" && (
            <span className="about-update-msg err">{t.about_check_error}</span>
          )}
        </div>

        <div className="about-links">
          <button className="about-link" onClick={() => ext(REPO)}>
            <ExternalLink size={16} /> {t.about_github}
          </button>
          <button className="about-link" onClick={() => ext(`${REPO}/releases`)}>
            <ExternalLink size={16} /> {t.about_releases}
          </button>
          <button className="about-link" onClick={() => ext(`${REPO}/issues`)}>
            <Bug size={16} /> {t.about_issues}
          </button>
        </div>

        <div className="about-foot">
          <div>{t.about_made_with}</div>
          <div className="about-copyright">{t.about_license}</div>
        </div>
      </div>
    </div>
  );
}
