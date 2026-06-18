export type Locale = "en" | "ru";

export type Strings = {
  // Navigation
  nav_translate: string;
  nav_history: string;
  nav_model: string;
  nav_settings: string;

  // Status bar
  model_ready: string;
  no_model: string;

  // Translator
  source_placeholder: string;
  auto_detect: string;
  translating: string;
  chars: (n: number) => string;
  swap_langs: string;
  copy_translation: string;
  clear: string;

  // Model manager
  onboarding_title: string;
  onboarding_subtitle: string;
  model_active: string;
  restart_engine: string;
  restarting: string;
  engine_error: string;
  retry: string;
  starting: string;
  downloading: string;
  cancel: string;
  available_variants: string;
  variants_hint: string;
  loaded_badge: string;
  load_btn: string;
  download_btn: string;
  delete_btn: string;
  cannot_delete_active: string;

  // History
  search_placeholder: string;
  clear_all: string;
  loading: string;
  no_matches: string;
  no_history: string;
  confirm_clear_history: string;

  // Settings sections
  settings_translation: string;
  settings_default_source: string;
  settings_default_target: string;
  settings_gpu: string;
  settings_gpu_hint: string;
  settings_glossary: string;
  settings_glossary_desc: string;
  settings_source_term: string;
  settings_translation_term: string;
  settings_add: string;
  settings_hotkeys: string;
  settings_triple_copy: string;
  settings_translate_replace: string;
  settings_triple_interval: string;
  settings_hotkey_hint: string;
  hotkey_press: string;
  settings_copy_taps: string;
  settings_copy_taps_hint: string;
  taps_double: string;
  taps_triple: string;
  settings_exclusions: string;
  settings_exclusions_btn: string;
  settings_exclusions_hint: string;
  exclusions_title: string;
  exclusions_desc: string;
  exclusions_current: string;
  exclusions_empty: string;
  exclusions_manual: string;
  exclusions_add: string;
  exclusions_running: string;
  exclusions_refresh: string;
  exclusions_none_running: string;
  exclusions_done: string;
  exclusions_close: string;
  exclusions_remove: string;
  settings_interface: string;
  settings_floating: string;
  settings_floating_hint: string;
  settings_autostart: string;
  settings_start_tray: string;
  settings_locale: string;
  settings_save: string;
  settings_saved: string;

  // Translate-replace banner
  translating_in_place: string;

  // Translation modes
  mode_label: string;
  mode_standard: string;
  mode_contextual: string;
  mode_formatted: string;
  mode_hint: string;

  // Divider
  divider_reset_hint: string;
};

const en: Strings = {
  nav_translate: "Translate",
  nav_history: "History",
  nav_model: "Model",
  nav_settings: "Settings",

  model_ready: "Model ready",
  no_model: "No model",

  source_placeholder: "Enter text to translate…",
  auto_detect: "Auto-detect",
  translating: "Translating…",
  chars: (n) => `${n} chars`,
  swap_langs: "Swap languages",
  copy_translation: "Copy translation",
  clear: "Clear",

  onboarding_title: "Welcome to DeepM",
  onboarding_subtitle:
    "Download the local translation model to get started. Your translations stay on your device — no internet required after setup.",
  model_active: "Model active:",
  restart_engine: "Restart engine",
  restarting: "Restarting…",
  engine_error: "Engine error",
  retry: "Retry",
  starting: "Starting…",
  downloading: "Downloading",
  cancel: "Cancel",
  available_variants: "Available variants",
  variants_hint: " — click Download to get, ✓ to load",
  loaded_badge: "● Loaded",
  load_btn: "Load",
  download_btn: "Download",
  delete_btn: "✕",
  cannot_delete_active: "Cannot delete active model",

  search_placeholder: "Search history…",
  clear_all: "Clear all",
  loading: "Loading…",
  no_matches: "No matches found.",
  no_history: "No translation history yet.",
  confirm_clear_history: "Clear all translation history?",

  settings_translation: "Translation",
  settings_default_source: "Default source language",
  settings_default_target: "Default target language",
  settings_gpu: "GPU acceleration",
  settings_gpu_hint: "Use CUDA if available",
  settings_glossary: "Glossary",
  settings_glossary_desc: "Terms here are passed to the model via terminology intervention.",
  settings_source_term: "Source term",
  settings_translation_term: "Translation",
  settings_add: "Add",
  settings_hotkeys: "Hotkeys",
  settings_triple_copy: "Triple-copy trigger",
  settings_translate_replace: "Translate & replace",
  settings_triple_interval: "Multi-copy interval (ms)",
  settings_hotkey_hint: "Click the field and press the key combination",
  hotkey_press: "Press keys…",
  settings_copy_taps: "Quick-copy presses",
  settings_copy_taps_hint: "How many fast Ctrl+C presses open DeepM with the copied text",
  taps_double: "Double (Ctrl+C ×2)",
  taps_triple: "Triple (Ctrl+C ×3)",
  settings_exclusions: "Excluded apps",
  settings_exclusions_btn: "Manage exclusions",
  settings_exclusions_hint: "Apps where the button and hotkeys are disabled",
  exclusions_title: "Excluded applications",
  exclusions_desc: "In these apps the floating button won't appear and DeepM's global hotkeys are ignored. Useful for terminals (e.g. MobaXterm) or games.",
  exclusions_current: "Excluded",
  exclusions_empty: "No apps excluded yet",
  exclusions_manual: "Add manually",
  exclusions_add: "Add",
  exclusions_running: "Running apps",
  exclusions_refresh: "Refresh",
  exclusions_none_running: "No other apps detected",
  exclusions_done: "Done",
  exclusions_close: "Close",
  exclusions_remove: "Remove",
  settings_interface: "Interface",
  settings_floating: "Show floating button",
  settings_floating_hint: "Appears when text is selected",
  settings_autostart: "Start with Windows",
  settings_start_tray: "Start in tray",
  settings_locale: "Interface language",
  settings_save: "Save settings",
  settings_saved: "Saved ✓",

  translating_in_place: "Translating in place…",

  mode_label: "Mode",
  mode_standard: "Standard",
  mode_contextual: "Contextual",
  mode_formatted: "Formatted",
  mode_hint: "Translation mode — Standard: plain direct translation · Contextual: feeds the previous text as context for consistent multi-part translations · Formatted: keeps markup/markdown tags intact",
  divider_reset_hint: "Drag to resize · double-click to centre",
};

const ru: Strings = {
  nav_translate: "Перевод",
  nav_history: "История",
  nav_model: "Модель",
  nav_settings: "Настройки",

  model_ready: "Модель готова",
  no_model: "Нет модели",

  source_placeholder: "Введите текст для перевода…",
  auto_detect: "Авто-определение",
  translating: "Перевод…",
  chars: (n) => `${n} симв.`,
  swap_langs: "Поменять языки",
  copy_translation: "Копировать перевод",
  clear: "Очистить",

  onboarding_title: "Добро пожаловать в DeepM",
  onboarding_subtitle:
    "Скачайте модель перевода для начала работы. Ваши переводы остаются на устройстве — интернет нужен только для скачивания.",
  model_active: "Активная модель:",
  restart_engine: "Перезапустить движок",
  restarting: "Перезапуск…",
  engine_error: "Ошибка движка",
  retry: "Повторить",
  starting: "Запуск…",
  downloading: "Скачивание",
  cancel: "Отмена",
  available_variants: "Доступные варианты",
  variants_hint: " — нажмите Скачать, или Загрузить если уже есть",
  loaded_badge: "● Загружено",
  load_btn: "Загрузить",
  download_btn: "Скачать",
  delete_btn: "✕",
  cannot_delete_active: "Нельзя удалить активную модель",

  search_placeholder: "Поиск по истории…",
  clear_all: "Очистить всё",
  loading: "Загрузка…",
  no_matches: "Ничего не найдено.",
  no_history: "История переводов пуста.",
  confirm_clear_history: "Удалить всю историю переводов?",

  settings_translation: "Перевод",
  settings_default_source: "Язык источника по умолчанию",
  settings_default_target: "Язык перевода по умолчанию",
  settings_gpu: "Ускорение GPU",
  settings_gpu_hint: "Использовать CUDA при наличии",
  settings_glossary: "Глоссарий",
  settings_glossary_desc: "Термины передаются модели как подсказки перевода.",
  settings_source_term: "Исходный термин",
  settings_translation_term: "Перевод",
  settings_add: "Добавить",
  settings_hotkeys: "Горячие клавиши",
  settings_triple_copy: "Тройной Ctrl+C",
  settings_translate_replace: "Перевод и замена",
  settings_triple_interval: "Интервал нажатий (мс)",
  settings_hotkey_hint: "Нажмите на поле и введите сочетание клавиш",
  hotkey_press: "Нажмите клавиши…",
  settings_copy_taps: "Нажатий Ctrl+C",
  settings_copy_taps_hint: "Сколько быстрых Ctrl+C открывают DeepM со скопированным текстом",
  taps_double: "Двойное (Ctrl+C ×2)",
  taps_triple: "Тройное (Ctrl+C ×3)",
  settings_exclusions: "Исключения",
  settings_exclusions_btn: "Управление исключениями",
  settings_exclusions_hint: "Приложения, где кнопка и горячие клавиши отключены",
  exclusions_title: "Исключённые приложения",
  exclusions_desc: "В этих приложениях плавающая кнопка не появляется, а глобальные горячие клавиши DeepM не срабатывают. Полезно для терминалов (например, MobaXterm) или игр.",
  exclusions_current: "Исключено",
  exclusions_empty: "Пока нет исключённых приложений",
  exclusions_manual: "Добавить вручную",
  exclusions_add: "Добавить",
  exclusions_running: "Запущенные приложения",
  exclusions_refresh: "Обновить",
  exclusions_none_running: "Другие приложения не найдены",
  exclusions_done: "Готово",
  exclusions_close: "Закрыть",
  exclusions_remove: "Удалить",
  settings_interface: "Интерфейс",
  settings_floating: "Плавающая кнопка",
  settings_floating_hint: "Появляется при выделении текста",
  settings_autostart: "Автозапуск с Windows",
  settings_start_tray: "Запускаться в трей",
  settings_locale: "Язык интерфейса",
  settings_save: "Сохранить",
  settings_saved: "Сохранено ✓",

  translating_in_place: "Перевод на месте…",

  mode_label: "Режим",
  mode_standard: "Стандарт",
  mode_contextual: "С контекстом",
  mode_formatted: "С разметкой",
  mode_hint: "Режим перевода — Стандарт: обычный прямой перевод · С контекстом: передаёт предыдущий текст как контекст для связного перевода частями · С разметкой: сохраняет теги разметки/markdown",
  divider_reset_hint: "Потяните для изменения · двойной клик — по центру",
};

const TRANSLATIONS: Record<Locale, Strings> = { en, ru };

export function getStrings(locale: Locale): Strings {
  return TRANSLATIONS[locale] ?? TRANSLATIONS.en;
}
