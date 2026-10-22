# Reproduce installer bitmaps from the approved production icon. Build-time only.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$projectRoot = if ($PSScriptRoot) { Split-Path -Parent $PSScriptRoot } else { (Get-Location).Path }
$assetDirectory = Join-Path $projectRoot 'src-tauri/installer'
New-Item -ItemType Directory -Force -Path $assetDirectory | Out-Null
$logo = [System.Drawing.Image]::FromFile((Join-Path $projectRoot 'src-tauri/icons/128x128@2x.png'))
$green = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#315c4d'))
$small = [System.Drawing.Font]::new('Segoe UI', 12, [System.Drawing.FontStyle]::Bold)
$large = [System.Drawing.Font]::new('Segoe UI', 18, [System.Drawing.FontStyle]::Regular)
try {
    foreach ($kind in @('header','sidebar')) {
        if ($kind -eq 'header') { $width=150; $height=57 } else { $width=164; $height=314 }
        $bitmap = [System.Drawing.Bitmap]::new($width,$height,[System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
            if ($kind -eq 'header') {
                $graphics.Clear([System.Drawing.Color]::White)
                $graphics.DrawImage($logo,8,10,36,36)
                $graphics.DrawString('DigView',$small,$green,47,17)
            } else {
                $graphics.Clear([System.Drawing.ColorTranslator]::FromHtml('#edf4f0'))
                $graphics.DrawImage($logo,30,45,104,104)
                $graphics.DrawString("Find.`nPreview.`nOrganize.",$large,$green,20,180)
            }
            $bitmap.Save((Join-Path $assetDirectory ($kind+'.bmp')),[System.Drawing.Imaging.ImageFormat]::Bmp)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    }
} finally { $logo.Dispose(); $green.Dispose(); $small.Dispose(); $large.Dispose() }
Write-Output 'DigView NSIS branding bitmaps prepared.'
