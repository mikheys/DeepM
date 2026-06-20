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
  - Fast data is downloaded via the jsdelivr CDN (github raw is often blocked).
  - ASCII-only on purpose so it parses regardless of the console code page.
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

# Try several mirrors in order (github raw is often TLS-blocked in some regions;
# the jsdelivr CDN usually works). Succeeds on the first non-empty download.
function DlMirror($urls, $out) {
  Write-Host "  -> $([IO.Path]::GetFileName($out))" -ForegroundColor DarkGray
  foreach ($u in $urls) {
    & curl.exe -L --retry 4 --retry-all-errors --retry-delay 2 --connect-timeout 20 -o $out $u 2>$null
    if ($LASTEXITCODE -eq 0 -and (Test-Path $out) -and (Get-Item $out).Length -gt 0) { return $true }
    Write-Host "     mirror failed, trying next" -ForegroundColor DarkYellow
  }
  return $false
}

# -- RapidOCR (PP-OCRv5 cyrillic) ----------------------------------------------
Write-Host "RapidOCR models (PP-OCRv5 cyrillic)" -ForegroundColor Cyan
New-Item -ItemType Directory -Force $rapid | Out-Null
$ms = "https://www.modelscope.cn/models/greatv/oar-ocr/resolve/master"
Dl "$ms/pp-ocrv5_mobile_det.onnx"          (Join-Path $rapid "det.onnx")
Dl "$ms/cyrillic_pp-ocrv5_mobile_rec.onnx" (Join-Path $rapid "rec.onnx")
Dl "$ms/ppocrv5_cyrillic_dict.txt"         (Join-Path $rapid "dict.txt")

# -- Tesseract (bundled exe + DLLs + standard data) ----------------------------
Write-Host "Tesseract (from $TesseractDir)" -ForegroundColor Cyan
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

# -- Tesseract fast data (CDN mirrors; falls back to copying standard) ----------
Write-Host "Tesseract fast data (tessdata_fast)" -ForegroundColor Cyan
function FastMirrors($lang) {
  @(
    "https://cdn.jsdelivr.net/gh/tesseract-ocr/tessdata_fast@main/$lang.traineddata",
    "https://fastly.jsdelivr.net/gh/tesseract-ocr/tessdata_fast@main/$lang.traineddata",
    "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/$lang.traineddata",
    "https://github.com/tesseract-ocr/tessdata_fast/raw/main/$lang.traineddata"
  )
}
foreach ($lang in @("eng","rus")) {
  $ok = DlMirror (FastMirrors $lang) (Join-Path $tessFast "$lang.traineddata")
  if (-not $ok) {
    Write-Host "  ! fast '$lang' unreachable - copying standard data as fallback" -ForegroundColor Yellow
    Copy-Item (Join-Path $tessStd "$lang.traineddata") (Join-Path $tessFast "$lang.traineddata") -Force
  }
}

# -- Summary -------------------------------------------------------------------
Write-Host "`nStaged:" -ForegroundColor Green
Get-ChildItem -Recurse $rapid, $tess | Where-Object { -not $_.PSIsContainer } |
  Select-Object @{n="File";e={$_.FullName.Substring($srcTauri.Length+1)}}, @{n="MB";e={[math]::Round($_.Length/1MB,2)}} |
  Format-Table -AutoSize
Write-Host "Done. Now run:  npm run tauri build" -ForegroundColor Green
