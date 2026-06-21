import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppSettings,
  ModelStatus,
  TranslationHistoryEntry,
} from "./types";

export type TranslateArgs = {
  source_text: string;
  source_lang: string;
  target_lang: string;
  context?: string;
  glossary_entries?: { source: string; target: string }[];
  mode?: string;
  style?: string;
};

export type TranslateResult = {
  translated_text: string;
  detected_lang?: string;
};

export async function translate(args: TranslateArgs): Promise<TranslateResult> {
  return invoke("translate", {
    sourceText: args.source_text,
    sourceLang: args.source_lang,
    targetLang: args.target_lang,
    context: args.context,
    glossaryEntries: args.glossary_entries,
    mode: args.mode ?? "standard",
    style: args.style,
  });
}

export async function getModelStatus(): Promise<ModelStatus> {
  return invoke("get_model_status");
}

export async function startModelDownload(
  version: string,
  size: string,
  quantization: string
): Promise<void> {
  return invoke("start_model_download", { version, size, quantization });
}

export async function cancelModelDownload(): Promise<void> {
  return invoke("cancel_model_download");
}

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke("save_settings", { settings });
}

export async function getHistory(): Promise<TranslationHistoryEntry[]> {
  return invoke("get_history");
}

export async function clearHistory(): Promise<void> {
  return invoke("clear_history");
}

export async function detectLanguage(text: string): Promise<string> {
  return invoke("detect_language", { text });
}

export function onDownloadProgress(
  callback: (progress: number, speed_mbps: number) => void
) {
  return listen<{ progress: number; speed_mbps: number }>(
    "download_progress",
    (e) => callback(e.payload.progress, e.payload.speed_mbps)
  );
}

export function onModelReady(callback: () => void) {
  return listen("model_ready", () => callback());
}

export function onModelError(callback: (msg: string) => void) {
  return listen<string>("model_error", (e) => callback(e.payload));
}

export function onModelDownloaded(callback: () => void) {
  return listen("model_downloaded", () => callback());
}

export function onDownloadCancelled(callback: () => void) {
  return listen("download_cancelled", () => callback());
}

export async function restartEngine(): Promise<void> {
  return invoke("restart_engine");
}

export async function loadModel(version: string, size: string, quantization: string): Promise<void> {
  return invoke("load_model", { version, size, quantization });
}

export async function loadExternalModel(path: string): Promise<void> {
  return invoke("load_external_model", { path });
}

export type DownloadState = {
  version: string;
  size: string;
  quantization: string;
  progress: number;
  speed_mbps: number;
};

export async function getDownloadState(): Promise<DownloadState | null> {
  return invoke("get_download_state");
}

export async function listDownloadedModels(): Promise<[string, string, string][]> {
  return invoke("list_downloaded_models");
}

export async function deleteModel(version: string, size: string, quantization: string): Promise<void> {
  return invoke("delete_model", { version, size, quantization });
}

export type GpuStatus = { cuda_ready: boolean; nvidia_present: boolean };

export async function gpuStatus(): Promise<GpuStatus> {
  return invoke("gpu_status");
}

// ── OCR (screenshot translation; bundled Tesseract) ───────────────────────
export async function ocrStatus(): Promise<boolean> {
  return invoke("ocr_status");
}
export async function ocrFromClipboard(): Promise<string> {
  return invoke("ocr_from_clipboard");
}
export async function ocrFromFile(path: string): Promise<string> {
  return invoke("ocr_from_file", { path });
}
/** Hidden diagnostic (no UI entry point) — sweeps tessdata x PSM. */
export async function ocrTestAll(path: string): Promise<import("./types").OcrTestResult[]> {
  return invoke("ocr_test_all", { path });
}
export async function ocrLangsStatus(): Promise<import("./types").OcrLang[]> {
  return invoke("ocr_langs_status");
}
export async function ocrLangDownload(code: string): Promise<boolean> {
  return invoke("ocr_lang_download", { code });
}
export async function ocrLangRemove(code: string): Promise<boolean> {
  return invoke("ocr_lang_remove", { code });
}
export async function launchSnip(): Promise<void> {
  return invoke("launch_snip");
}

export async function listAppProcesses(): Promise<string[]> {
  return invoke("list_app_processes");
}
