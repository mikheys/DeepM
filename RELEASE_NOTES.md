<div align="center">

## DeepM v0.2.0

**A private, fully-local translator for Windows — a DeepL alternative.**
**Приватный локальный переводчик для Windows — аналог DeepL.**

[English](#english) · [Русский](#russian)

</div>

---

<a name="english"></a>
### English

Powered by Tencent HY-MT via llama.cpp. Everything runs on your device — the internet is only needed to download the model once.

#### What's new in 0.2.0

- **OCR reworked — bundled Tesseract (rus+eng).** Screenshot/image OCR works out of the box, **nothing to install separately**. The old Windows OCR and experimental RapidOCR backends were dropped after benchmarking (Tesseract was the most accurate *and* fastest on mixed RU/EN). Plus light normalization that repairs mixed Cyrillic/Latin look-alikes (`ОСR → OCR`).
- **Link Mode (experimental)** — click a sentence/paragraph on one side to highlight its counterpart on the other (length-based alignment, fully offline).
- **Translate & Replace fixes** — no more stray `⌀` characters; new default hotkey **`Ctrl+Shift+Alt+T`**. The floating popup gained a **Replace** button.
- New application icon; smaller, simpler installer.

**Features:** two-pane translator with auto RU↔EN direction, floating button on text selection, translate-&-replace hotkey, multi-copy to window, translation modes, history, glossary, per-app exclusions, screenshot/image OCR, tray & autostart.

#### What to download

| File | For whom | Size |
|------|----------|------|
| **DeepM_0.2.0_x64-setup.exe** | **Everyone** — CPU build, any Windows 10/11 x64. | ~80 MB |
| **DeepM-cuda-pack.zip** | NVIDIA acceleration — full, self-contained. | ~764 MB |
| **DeepM-cuda-pack-slim.zip** | NVIDIA acceleration **if you already have CUDA Toolkit 12.x**. | ~307 MB |

#### Install

1. Run **DeepM_0.2.0_x64-setup.exe**. On SmartScreen: *More info → Run anyway* (not code-signed yet).
2. Open **Models** and download a model (start with **1.8B Q4_K_M**, ~1.1 GB). Downloaded once, stored locally.
3. Select text and translate.

#### NVIDIA GPU (optional)

The app auto-detects your GPU: the *Use GPU* toggle is only active when acceleration will actually work. To enable it, extract the `engine` folder from a CUDA pack into the install dir (e.g. `C:\Program Files\DeepM\`), restart DeepM, then tick *Use GPU* and *Reload engine*. Use the **slim** pack only if you already have CUDA Toolkit 12.x installed; otherwise use the **full** pack.

> `ggml-cuda.dll` (~540 MB) is llama.cpp's own CUDA backend and is never part of a system CUDA install, so it must ship. Only `cublas`/`cudart` (~547 MB) can come from a system CUDA Toolkit — that's what the slim pack relies on.

**Requirements:** Windows 10/11 x64 · WebView2 (auto-installed if missing) · model downloaded in-app (not in the installer).

---

<a name="russian"></a>
### Русский

На базе Tencent HY-MT через llama.cpp. Всё работает на вашем устройстве — интернет нужен только для однократной загрузки модели.

#### Что нового в 0.2.0

- **OCR переработан — встроенный Tesseract (рус+англ).** Распознавание со скриншотов/картинок работает сразу, **ничего доустанавливать не нужно**. Старый Windows OCR и экспериментальный RapidOCR убраны после замеров (Tesseract оказался точнее *и* быстрее на смешанном RU/EN). Плюс лёгкая нормализация, чинящая смешанные кириллица/латиница-омоглифы (`ОСR → OCR`).
- **Режим связи (экспериментальный)** — клик по предложению/абзацу с одной стороны подсвечивает соответствие с другой (выравнивание по длине, полностью офлайн).
- **Исправления перевода-замены** — больше нет паразитных символов `⌀`; новый дефолтный хоткей **`Ctrl+Shift+Alt+T`**. У плавающего окна появилась кнопка **«Заменить»**.
- Новая иконка; установщик меньше и проще.

**Возможности:** двухпанельный переводчик с авто-направлением RU↔EN, плавающая кнопка по выделению, перевод-замена по горячей клавише, множественный Ctrl+C в окно, режимы перевода, история, глоссарий, исключения приложений, OCR скриншотов/картинок, трей и автозапуск.

#### Что скачать

| Файл | Для кого | Размер |
|------|----------|--------|
| **DeepM_0.2.0_x64-setup.exe** | **Для всех** — CPU-сборка, любой Windows 10/11 x64. | ~80 МБ |
| **DeepM-cuda-pack.zip** | Ускорение NVIDIA — полный, ни от чего не зависит. | ~764 МБ |
| **DeepM-cuda-pack-slim.zip** | Ускорение NVIDIA, **если уже стоит CUDA Toolkit 12.x**. | ~307 МБ |

#### Установка

1. Запустите **DeepM_0.2.0_x64-setup.exe**. В SmartScreen: *Подробнее → Выполнить в любом случае* (приложение пока без подписи).
2. Откройте **Модели** и скачайте модель (начните с **1.8B Q4_K_M**, ~1.1 ГБ). Качается один раз, хранится локально.
3. Выделяйте текст и переводите.

#### Видеокарта NVIDIA (необязательно)

Приложение само определяет видеокарту: галочка *Использовать GPU* активна, только если ускорение реально заработает. Чтобы включить — распакуйте папку `engine` из CUDA-пакета в каталог установки (например, `C:\Program Files\DeepM\`), перезапустите DeepM, поставьте галочку и нажмите *Перезагрузить движок*. **Slim**-пакет берите, только если у вас уже установлен CUDA Toolkit 12.x; иначе — **полный** пакет.

> `ggml-cuda.dll` (~540 МБ) — это CUDA-бэкенд самого llama.cpp, его нет в системной установке CUDA, поэтому он всегда в пакете. Из системного CUDA Toolkit могут браться только `cublas`/`cudart` (~547 МБ) — на этом и построен slim-вариант.

**Требования:** Windows 10/11 x64 · WebView2 (установится автоматически) · модель скачивается внутри приложения (не входит в установщик).

---

🤖 Generated with [Claude Code](https://claude.com/claude-code)
