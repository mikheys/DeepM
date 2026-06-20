<#
.SYNOPSIS
  Stages the OCR resources that get bundled into the DeepM installer:
    src-tauri/rapidocr/    -> det.onnx / rec.onnx / dict.txt  (PP-OCRv5 cyrillic)
    src-tauri/tesseract/   -> tesseract.exe + *.dll
                              tessdata-standard/{eng,rus}.traineddata
                              tessdata-fast/{eng,rus}.traineddata

  These folders are .gitignored (like src-tauri/engine), so run this once on the
  build machine BEFORE `npm run tauri build`.

.NOTES
  - RapidOCR models come from ModelScope greatv/oar-ocr (resolve URLs).
  - Tesseract exe + DLLs + standard data are copied from a local Tesseract 5
    install (UB-Mannheim). Override the path with -TesseractDir.
  - Fast data is downloaded from the tessdata_fast repo.
#>
param(
  [string]$TesseractDir = "C:\Program Files\Tesseract-OCR"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot              # repo root
$srcTauri = Join-Path $root "src-tauri"
$rapid = Join-Path $srcTauri "rapidocr"
$tess  = Join-Path $srcTauri "tesseract"
$tessStd  = Join-Path $tess "tessdata-standard"
$tessFast = Join-Path $tess "tessdata-fast"

function Dl($url, $out) {
  Write-Host "  -> $([IO.Path]::GetFileName($out))" -ForegroundColor DarkGray
  & curl.exe -L --retry 10 --retry-all-errors --retry-delay 3 --connect-timeout 30 -o $out $url
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path $out)) { throw "download failed: $url" }
}

# ── RapidOCR (PP-OCRv5 cyrillic) ───────────────────────────────────────────────
Write-Host "RapidOCR models (PP-OCRv5 cyrillic)…" -ForegroundColor Cyan
New-Item -ItemType Directory -Force $rapid | Out-Null
$ms = "https://www.modelscope.cn/models/greatv/oar-ocr/resolve/master"
Dl "$ms/pp-ocrv5_mobile_det.onnx"        (Join-Path $rapid "det.onnx")
Dl "$ms/cyrillic_pp-ocrv5_mobile_rec.onnx" (Join-Path $rapid "rec.onnx")
Dl "$ms/ppocrv5_cyrillic_dict.txt"       (Join-Path $rapid "dict.txt")

# ── Tesseract (bundled exe + DLLs + standard data) ─────────────────────────────
Write-Host "Tesseract (from $TesseractDir)…" -ForegroundColor Cyan
if (-not (Test-Path (Join-Path $TesseractDir "tesseract.exe"))) {
  throw "tesseract.exe not found in '$TesseractDir'. Install UB-Mannheim Tesseract (with Russian) or pass -TesseractDir."
}
New-Item -ItemType Directory -Force $tess, $tessStd, $tessFast | Out-Null
Copy-Item (Join-Path $TesseractDir "tesseract.exe") $tess -Force
Get-ChildItem (Join-Path $TesseractDir "*.dll") | Copy-Item -Destination $tess -Force
foreach ($lang in @("eng","rus")) {
  $src = Join-Path $TesseractDir "tessdata\$lang.traineddata"
  if (-not (Test-Path $src)) { throw "Missing $lang.traineddata in install. Re-run Tesseract setup and add the '$lang' language." }
  Copy-Item $src $tessStd -Force
}

# ── Tesseract fast data ────────────────────────────────────────────────────────
Write-Host "Tesseract fast data (tessdata_fast)…" -ForegroundColor Cyan
$tf = "https://github.com/tesseract-ocr/tessdata_fast/raw/main"
Dl "$tf/eng.traineddata" (Join-Path $tessFast "eng.traineddata")
Dl "$tf/rus.traineddata" (Join-Path $tessFast "rus.traineddata")

# ── Summary ────────────────────────────────────────────────────────────────────
Write-Host "`nStaged:" -ForegroundColor Green
Get-ChildItem -Recurse $rapid, $tess | Where-Object { -not $_.PSIsContainer } |
  Select-Object @{n="File";e={$_.FullName.Substring($srcTauri.Length+1)}}, @{n="MB";e={[math]::Round($_.Length/1MB,2)}} |
  Format-Table -AutoSize
Write-Host "Done. Now run:  npm run tauri build" -ForegroundColor Green
