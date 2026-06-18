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
  formatted?: boolean;
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
    formatted: args.formatted ?? false,
  });
}

export async function getModelStatus(): Promise<ModelStatus> {
  return invoke("get_model_status");
}

export async function startModelDownload(
  size: string,
  quantization: string
): Promise<void> {
  return invoke("start_model_download", { size, quantization });
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

export async function loadModel(size: string, quantization: string): Promise<void> {
  return invoke("load_model", { size, quantization });
}

export async function loadExternalModel(path: string): Promise<void> {
  return invoke("load_external_model", { path });
}

export type DownloadState = {
  size: string;
  quantization: string;
  progress: number;
  speed_mbps: number;
};

export async function getDownloadState(): Promise<DownloadState | null> {
  return invoke("get_download_state");
}

export async function listDownloadedModels(): Promise<[string, string][]> {
  return invoke("list_downloaded_models");
}

export async function deleteModel(size: string, quantization: string): Promise<void> {
  return invoke("delete_model", { size, quantization });
}

export async function isCudaAvailable(): Promise<boolean> {
  return invoke("is_cuda_available");
}

export async function listAppProcesses(): Promise<string[]> {
  return invoke("list_app_processes");
}
