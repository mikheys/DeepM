<#
.SYNOPSIS
  Stages the OCR resource bundled into the DeepM installer:
    src-tauri/tesseract/   -> tesseract.exe
                              + the DLLs needed for recognition (trimmed)
                              + tessdata-standard/{eng,rus}.traineddata

  This folder is .gitignored (like src-tauri/engine), so run this once on the
  build machine BEFORE `npm run tauri build`.

.PARAMETER TesseractDir
  Source Tesseract 5 install (UB-Mannheim). Default: C:\Program Files\Tesseract-OCR

.PARAMETER AllDlls
  Copy ALL DLLs from the install instead of the trimmed recognition set. Use
  this if OCR fails after a trimmed copy (some dependency was excluded).

.NOTES
  ASCII-only so it parses regardless of the console code page.
#>
param(
  [string]$TesseractDir = "C:\Program Files\Tesseract-OCR",
  [switch]$AllDlls
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$srcTauri = Join-Path $root "src-tauri"
$tess     = Join-Path $srcTauri "tesseract"
$tessStd  = Join-Path $tess "tessdata-standard"

# DLLs actually needed for OCR recognition. The bulky rendering stack (pango,
# cairo, glib, fonts), networking (curl, ssl, ssh2) and archive libs that
# UB-Mannheim ships are for training/utilities, not recognition, so they are
# dropped. Matched by prefix so version suffixes don't matter.
$keep = @(
  "libtesseract*","libleptonica*",
  "libgcc_s_seh*","libstdc++*","libwinpthread*",
  "libicudt*","libicuin*","libicuuc*",
  "libintl*","libiconv*",
  "libpng*","libjpeg*","libtiff*","libwebp*","libwebpmux*","libsharpyuv*",
  "libgif*","libopenjp2*","zlib*","libLerc*","libdeflate*","libjbig*",
  "liblzma*","libzstd*"
)

if (-not (Test-Path (Join-Path $TesseractDir "tesseract.exe"))) {
  throw "tesseract.exe not found in '$TesseractDir'. Install UB-Mannheim Tesseract (with Russian) or pass -TesseractDir."
}

Write-Host "Staging Tesseract from $TesseractDir" -ForegroundColor Cyan
# Start clean so a previous (untrimmed) copy doesn't leave extra DLLs behind.
if (Test-Path $tess) { Remove-Item -Recurse -Force $tess }
New-Item -ItemType Directory -Force $tess, $tessStd | Out-Null

Copy-Item (Join-Path $TesseractDir "tesseract.exe") $tess -Force

$allDll = Get-ChildItem (Join-Path $TesseractDir "*.dll")
if ($AllDlls) {
  $allDll | Copy-Item -Destination $tess -Force
  Write-Host "  copied ALL $($allDll.Count) DLLs" -ForegroundColor DarkGray
} else {
  $copied = 0
  foreach ($dll in $allDll) {
    foreach ($pat in $keep) {
      if ($dll.Name -like "$pat.dll") { Copy-Item $dll.FullName $tess -Force; $copied++; break }
    }
  }
  Write-Host "  copied $copied of $($allDll.Count) DLLs (trimmed; pass -AllDlls if OCR fails)" -ForegroundColor DarkGray
}

foreach ($lang in @("eng","rus")) {
  $src = Join-Path $TesseractDir "tessdata\$lang.traineddata"
  if (-not (Test-Path $src)) { throw "Missing $lang.traineddata in install. Re-run Tesseract setup and add the '$lang' language." }
  Copy-Item $src $tessStd -Force
}

Write-Host "`nStaged:" -ForegroundColor Green
$bytes = (Get-ChildItem -Recurse $tess | Where-Object { -not $_.PSIsContainer } | Measure-Object Length -Sum).Sum
Get-ChildItem -Recurse $tess | Where-Object { -not $_.PSIsContainer } |
  Select-Object @{n="File";e={$_.FullName.Substring($srcTauri.Length+1)}}, @{n="MB";e={[math]::Round($_.Length/1MB,2)}} |
  Format-Table -AutoSize
Write-Host ("Total bundled: {0:N1} MB" -f ($bytes/1MB)) -ForegroundColor Green
Write-Host "Done. Now run:  npm run tauri build" -ForegroundColor Green
