export type Language = {
  code: string;
  name: string;
  nativeName: string;
};

export const LANGUAGES: Language[] = [
  { code: "auto", name: "Auto-detect", nativeName: "Auto" },
  { code: "en", name: "English", nativeName: "English" },
  { code: "ru", name: "Russian", nativeName: "Русский" },
  { code: "zh", name: "Chinese (Simplified)", nativeName: "中文(简体)" },
  { code: "zh-TW", name: "Chinese (Traditional)", nativeName: "中文(繁體)" },
  { code: "yue", name: "Cantonese", nativeName: "粤语" },
  { code: "fr", name: "French", nativeName: "Français" },
  { code: "de", name: "German", nativeName: "Deutsch" },
  { code: "es", name: "Spanish", nativeName: "Español" },
  { code: "pt", name: "Portuguese", nativeName: "Português" },
  { code: "it", name: "Italian", nativeName: "Italiano" },
  { code: "nl", name: "Dutch", nativeName: "Nederlands" },
  { code: "pl", name: "Polish", nativeName: "Polski" },
  { code: "cs", name: "Czech", nativeName: "Čeština" },
  { code: "uk", name: "Ukrainian", nativeName: "Українська" },
  { code: "ja", name: "Japanese", nativeName: "日本語" },
  { code: "ko", name: "Korean", nativeName: "한국어" },
  { code: "ar", name: "Arabic", nativeName: "العربية" },
  { code: "he", name: "Hebrew", nativeName: "עברית" },
  { code: "fa", name: "Persian", nativeName: "فارسی" },
  { code: "tr", name: "Turkish", nativeName: "Türkçe" },
  { code: "th", name: "Thai", nativeName: "ไทย" },
  { code: "vi", name: "Vietnamese", nativeName: "Tiếng Việt" },
  { code: "ms", name: "Malay", nativeName: "Melayu" },
  { code: "id", name: "Indonesian", nativeName: "Indonesia" },
  { code: "tl", name: "Filipino", nativeName: "Filipino" },
  { code: "hi", name: "Hindi", nativeName: "हिंदी" },
  { code: "bn", name: "Bengali", nativeName: "বাংলা" },
  { code: "gu", name: "Gujarati", nativeName: "ગુજરાતી" },
  { code: "ur", name: "Urdu", nativeName: "اردو" },
  { code: "te", name: "Telugu", nativeName: "తెలుగు" },
  { code: "mr", name: "Marathi", nativeName: "मराठी" },
  { code: "ta", name: "Tamil", nativeName: "தமிழ்" },
  { code: "km", name: "Khmer", nativeName: "ខ្មែរ" },
  { code: "my", name: "Burmese", nativeName: "မြန်မာ" },
  { code: "kk", name: "Kazakh", nativeName: "Қазақша" },
  { code: "mn", name: "Mongolian", nativeName: "Монгол" },
  { code: "ug", name: "Uyghur", nativeName: "ئۇيغۇرچە" },
  { code: "bo", name: "Tibetan", nativeName: "བོད་སྐད།" },
];

export const TARGET_LANGUAGES: Language[] = [
  { code: "auto", name: "Auto (EN↔RU)", nativeName: "Авто" },
  ...LANGUAGES.filter((l) => l.code !== "auto"),
];

export type ModelSize = "1.8B" | "7B";
export type Quantization = "Q4_K_M" | "Q6_K" | "Q8_0";

export type ModelConfig = {
  size: ModelSize;
  quantization: Quantization;
  path: string;
};

export type ModelStatus =
  | { type: "not_downloaded" }
  | { type: "downloading"; progress: number; speed_mbps: number }
  | { type: "ready"; path: string }
  | { type: "error"; message: string };

export type TranslationMode =
  | "standard"
  | "terminology"
  | "contextual"
  | "formatted"
  | "style"
  | "structured"
  | "delimiter";

export type GlossaryEntry = {
  id: string;
  source: string;
  target: string;
  lang_pair: string; // e.g. "en->zh"
};

export type TranslationHistoryEntry = {
  id: string;
  timestamp: number;
  source_lang: string;
  target_lang: string;
  source_text: string;
  translated_text: string;
};

export type AppSettings = {
  default_source_lang: string;
  default_target_lang: string;
  use_gpu: boolean;
  model_version: string;
  model_size: ModelSize;
  quantization: Quantization;
  model_path: string;
  glossary: GlossaryEntry[];
  hotkeys: {
    triple_copy: string;
    translate_replace: string;
  };
  show_floating_button: boolean;
  autostart: boolean;
  start_in_tray: boolean;
  triple_copy_interval_ms: number;
  triple_copy_count: number;
  floating_exclusions: string[];
  /** Tesseract languages always used for OCR (e.g. ["rus","eng"]). */
  ocr_languages: string[];
  /** Auto-detect the image script and add/download the matching language. */
  ocr_auto_lang: boolean;
  locale: string;
};

export type OcrLang = {
  code: string;
  name: string;
  installed: boolean;
  enabled: boolean;
  bundled: boolean;
};

export type AppView =
  | "translator" | "settings" | "history" | "model_manager" | "onboarding" | "ocr_test" | "about";

export type OcrTestResult = {
  engine: string;
  model: string;
  preprocess: string;
  ms: number;
  text: string;
  normalized: string;
  error: string | null;
};
