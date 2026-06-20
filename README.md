<div align="center">

# DeepM

**A private, fully‑local translator for Windows — a DeepL alternative that runs entirely on your machine.**
**Приватный локальный переводчик для Windows — аналог DeepL, работающий полностью на вашем устройстве.**

Powered by [Tencent HY‑MT1.5](https://huggingface.co/tencent) via [llama.cpp](https://github.com/ggerganov/llama.cpp) · Tauri 2 · React · Rust

[English](#english) · [Русский](#русский)

</div>

---

<a name="english"></a>

## English

All translation happens on your device. The internet is only needed to download the model once.

### Features

- **Translator panel** — two‑pane UI, draggable divider (double‑click to center), language swap, automatic RU↔EN direction.
- **Floating button** — select text in any app and a small button appears next to it; click to translate. Only shows up when text is actually selected.
- **Translate & Replace** — a hotkey (default `Ctrl+Shift+Alt+T`) translates the selected text in place.
- **Multi‑copy to app** — press `Ctrl+C` quickly 2 or 3 times (configurable) to send text to the translator window.
- **Translation modes** — *Standard* (direct), *Contextual* (uses previous text), *Formatted* (keeps markup tags).
- **History & glossary** — stored locally; custom terms are passed to the model.
- **App exclusions** — disable the button/hotkeys in chosen apps (e.g. terminals, games).
- **Screenshot / image OCR** — translate text you can't select: snip a screen
  area, OCR a screenshot from the clipboard, or pick an image file (offline,
  via the built-in Windows OCR engine).
- **Tray & autostart** — runs in the background; optional start with Windows.

### Install (end users)

Grab the latest [**Release**](https://github.com/mikheys/DeepM/releases/latest):

| File | For whom | Size |
|------|----------|------|
| `DeepM_x.x.x_x64-setup.exe` | **Everyone** — CPU build, any Windows 10/11 x64. | ~12 MB |
| `DeepM-cuda-pack.zip` | NVIDIA acceleration — full, self‑contained. | ~764 MB |
| `DeepM-cuda-pack-slim.zip` | NVIDIA acceleration **if you already have CUDA Toolkit 12.x**. | ~307 MB |

1. Run the installer. On SmartScreen: *More info → Run anyway* (the app isn't code‑signed yet).
2. Open **Models** and download a model (start with **1.8B Q4_K_M**, ~1.1 GB). It's stored locally and downloaded once.
3. Select text and translate.

### CPU vs GPU — how it actually works

The model is a **GGUF file** loaded by a bundled `llama-server`. The **"Use GPU"** checkbox controls one thing: whether the model's layers are offloaded to an NVIDIA GPU (`--n-gpu-layers`).

| | Where weights live | Compute | Speed | Needs |
|---|---|---|---|---|
| **CPU mode** (GPU off) | System **RAM** | CPU cores | Slower, but universal | nothing extra |
| **GPU mode** (GPU on) | GPU **VRAM** | NVIDIA GPU | Much faster (esp. 7B) | NVIDIA + CUDA backend |

The same model file is used either way — only *where* it runs differs.

**Why the toggle may be greyed out:** GPU mode needs `ggml-cuda.dll` (llama.cpp's CUDA backend, ~540 MB — part of the GPU pack, never provided by a system CUDA install) plus the CUDA runtime (`cublas`/`cublasLt`/`cudart`, which can come from the GPU pack **or** a system CUDA Toolkit 12.x). DeepM checks this automatically:

- **GPU pack installed + NVIDIA present** → checkbox enabled.
- **NVIDIA present, no GPU pack** → "install the CUDA pack to enable".
- **No NVIDIA GPU** → "GPU acceleration unavailable".

**Enable GPU:** extract the `engine` folder from a CUDA pack into the install dir (e.g. `C:\Program Files\DeepM\`), restart DeepM, tick *Use GPU* in Settings, then *Reload engine* in Models.

**Test each mode:** toggle *Use GPU* → Save → *Reload engine*. Watch **Task Manager → Performance**: GPU mode raises **GPU / VRAM** usage; CPU mode raises **Memory (RAM)** and leaves the GPU idle.

### Build from source (developers)

Requirements: [Node.js 18+](https://nodejs.org/), [Rust](https://rustup.rs/), Windows 10/11 x64.

```bash
git clone https://github.com/mikheys/DeepM.git
cd DeepM
npm install
```

Download llama.cpp Windows binaries from its [releases](https://github.com/ggerganov/llama.cpp/releases/latest) and extract **all** files into `src-tauri/binaries/` (the `cuda-12.x` build for GPU, or the plain `x64` build for CPU). `llama-server.exe` needs its companion DLLs in the same folder.

```bash
npm run tauri dev      # development
npm run tauri build    # release installer (bundles src-tauri/engine/)
```

> The release build bundles a **CPU‑only** engine staged in `src-tauri/engine/`. The full GPU `binaries/` folder is used only in `dev` builds. See [`RELEASE_NOTES.md`](RELEASE_NOTES.md) for packaging details (CPU installer + full/slim CUDA packs).

### Architecture

```
DeepM/
├── src/                      # React + TypeScript (frontend)
│   ├── components/           # UI components
│   └── api.ts                # Tauri IPC bindings
└── src-tauri/                # Rust (backend)
    ├── src/
    │   ├── core/             # engine, model manager, history, prompts
    │   ├── os_integration/   # tray, floating button, clipboard, hotkeys, processes
    │   └── lib.rs            # entry point + Tauri commands
    ├── binaries/             # full llama.cpp set incl. CUDA (dev, not in git)
    └── engine/               # CPU subset staged for bundling (not in git)
```

**Stack:** Tauri 2 · React 18 · TypeScript · Vite 6 · Rust · llama.cpp

### Models

Two Tencent translation families are downloadable in-app:

- **HY-MT1.5** — the original, proven model.
- **Hy-MT2** — newer: 33 languages and more translation modes (terminology, style,
  context, structured-data protection). Recommended.

Each comes in 1.8B and 7B sizes with Q4_K_M / Q6_K / Q8_0 quants. You can also
load any external `.gguf` finetune from disk.

| Size / quant | Quality | VRAM / RAM |
|--------------|---------|------------|
| 1.8B Q4_K_M | Good, fast | ~1.1 GB |
| 1.8B Q8_0 | Better | ~1.9 GB |
| 7B Q4_K_M | Best | ~4.6 GB |

### License

MIT

---

<a name="русский"></a>

## Русский

Все переводы выполняются на вашем устройстве. Интернет нужен только для однократной загрузки модели.

### Возможности

- **Панель перевода** — две панели, перетаскиваемый разделитель (двойной клик — по центру), обмен языков, авто‑направление RU↔EN.
- **Плавающая кнопка** — выделите текст в любом приложении, рядом появится кнопка; клик — перевод. Появляется только когда текст действительно выделен.
- **Перевод и замена** — горячая клавиша (по умолчанию `Ctrl+Shift+Alt+T`) переводит выделенный текст прямо на месте.
- **Множественный Ctrl+C** — нажмите `Ctrl+C` быстро 2 или 3 раза (настраивается), чтобы отправить текст в окно переводчика.
- **Режимы перевода** — *Стандарт* (прямой), *С контекстом* (учитывает предыдущий текст), *С разметкой* (сохраняет теги).
- **История и глоссарий** — хранятся локально; свои термины передаются модели.
- **Исключения приложений** — отключить кнопку/хоткеи в выбранных программах (терминалы, игры).
- **OCR скриншотов / картинок** — перевод невыделяемого текста: снимок области
  экрана, распознавание скриншота из буфера или выбор файла изображения
  (офлайн, через встроенный в Windows OCR-движок).
- **Трей и автозапуск** — работает в фоне; опциональный старт с Windows.

### Установка (для пользователей)

Скачайте из последнего [**релиза**](https://github.com/mikheys/DeepM/releases/latest):

| Файл | Для кого | Размер |
|------|----------|--------|
| `DeepM_x.x.x_x64-setup.exe` | **Для всех** — CPU‑сборка, любой Windows 10/11 x64. | ~12 МБ |
| `DeepM-cuda-pack.zip` | Ускорение NVIDIA — полный, ни от чего не зависит. | ~764 МБ |
| `DeepM-cuda-pack-slim.zip` | Ускорение NVIDIA, **если уже стоит CUDA Toolkit 12.x**. | ~307 МБ |

1. Запустите установщик. В SmartScreen: *Подробнее → Выполнить в любом случае* (приложение пока без подписи).
2. Откройте **Модели** и скачайте модель (начните с **1.8B Q4_K_M**, ~1.1 ГБ). Хранится локально, качается один раз.
3. Выделяйте текст и переводите.

### CPU и GPU — как это устроено

Модель — это **GGUF‑файл**, который запускает встроенный `llama-server`. Галочка **«Использовать GPU»** управляет одним: выгружать ли слои модели на видеокарту NVIDIA (`--n-gpu-layers`).

| | Где веса | Вычисления | Скорость | Что нужно |
|---|---|---|---|---|
| **CPU‑режим** (GPU выкл.) | Оперативная память (**RAM**) | Ядра процессора | Медленнее, но универсально | ничего сверх |
| **GPU‑режим** (GPU вкл.) | Видеопамять (**VRAM**) | Видеокарта NVIDIA | Намного быстрее (особенно 7B) | NVIDIA + CUDA‑бэкенд |

Файл модели один и тот же — отличается только *где* он работает.

**Почему галочка может быть неактивна:** для GPU нужен `ggml-cuda.dll` (CUDA‑бэкенд самого llama.cpp, ~540 МБ — часть GPU‑пакета, в системной CUDA его нет никогда) плюс рантайм CUDA (`cublas`/`cublasLt`/`cudart` — берётся из GPU‑пакета **или** из системного CUDA Toolkit 12.x). DeepM проверяет это автоматически:

- **GPU‑пакет установлен + есть NVIDIA** → галочка активна.
- **NVIDIA есть, GPU‑пакета нет** → «установите CUDA‑пакет для активации».
- **Видеокарты NVIDIA нет** → «ускорение GPU недоступно».

**Включить GPU:** распакуйте папку `engine` из CUDA‑пакета в каталог установки (например, `C:\Program Files\DeepM\`), перезапустите DeepM, поставьте галочку *Использовать GPU* в Настройках и нажмите *Перезагрузить движок* в Моделях.

**Проверить каждый режим:** переключите *Использовать GPU* → Сохранить → *Перезагрузить движок*. Смотрите **Диспетчер задач → Производительность**: в GPU‑режиме растёт **GPU / VRAM**, в CPU‑режиме растёт **Память (RAM)**, а видеокарта простаивает.

### Сборка из исходников (для разработчиков)

Требования: [Node.js 18+](https://nodejs.org/), [Rust](https://rustup.rs/), Windows 10/11 x64.

```bash
git clone https://github.com/mikheys/DeepM.git
cd DeepM
npm install
```

Скачайте Windows‑бинарники llama.cpp из его [релизов](https://github.com/ggerganov/llama.cpp/releases/latest) и распакуйте **все** файлы в `src-tauri/binaries/` (сборка `cuda-12.x` для GPU или обычная `x64` для CPU). `llama-server.exe` требует сопутствующих DLL в той же папке.

```bash
npm run tauri dev      # разработка
npm run tauri build    # установщик релиза (бандлит src-tauri/engine/)
```

> Релизная сборка включает **только CPU‑движок** из `src-tauri/engine/`. Полная GPU‑папка `binaries/` используется лишь в `dev`. Детали упаковки (CPU‑установщик + полный/slim CUDA‑пакеты) — в [`RELEASE_NOTES.md`](RELEASE_NOTES.md).

### Архитектура

```
DeepM/
├── src/                      # React + TypeScript (фронтенд)
│   ├── components/           # UI‑компоненты
│   └── api.ts                # Tauri IPC биндинги
└── src-tauri/                # Rust (бэкенд)
    ├── src/
    │   ├── core/             # движок, менеджер моделей, история, промпты
    │   ├── os_integration/   # трей, плавающая кнопка, буфер, хоткеи, процессы
    │   └── lib.rs            # точка входа + Tauri‑команды
    ├── binaries/             # полный набор llama.cpp с CUDA (dev, не в git)
    └── engine/               # CPU‑набор для бандла (не в git)
```

**Стек:** Tauri 2 · React 18 · TypeScript · Vite 6 · Rust · llama.cpp

### Модели

В приложении можно скачать два семейства переводчиков Tencent:

- **HY-MT1.5** — оригинальная, проверенная модель.
- **Hy-MT2** — новее: 33 языка и больше режимов перевода (терминология, стиль,
  контекст, защита структурных данных). Рекомендуется.

У каждой — размеры 1.8B и 7B и кванты Q4_K_M / Q6_K / Q8_0. Также можно загрузить
любой свой `.gguf` файнтюн с диска.

| Размер / квант | Качество | VRAM / RAM |
|----------------|----------|------------|
| 1.8B Q4_K_M | Хорошее, быстрое | ~1.1 ГБ |
| 1.8B Q8_0 | Лучше | ~1.9 ГБ |
| 7B Q4_K_M | Наилучшее | ~4.6 ГБ |

### Лицензия

MIT
