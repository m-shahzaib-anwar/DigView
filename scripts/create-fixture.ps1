# Generated development data only; this never reads the user's folders.
$ErrorActionPreference = 'Stop'
$fixtureRoot = Join-Path (Split-Path $PSScriptRoot -Parent) '.tools/e2e-library'
New-Item -ItemType Directory -Force -Path $fixtureRoot | Out-Null
Add-Type -AssemblyName System.Drawing
$bitmap = [System.Drawing.Bitmap]::new(1200, 300)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$font = [System.Drawing.Font]::new('Arial', 42)
try {
    $graphics.Clear([System.Drawing.Color]::White)
    $graphics.DrawString('DigView 2026 - private files', $font, [System.Drawing.Brushes]::Black, 30, 100)
    $bitmap.Save((Join-Path $fixtureRoot 'Screenshot - OCR.png'), [System.Drawing.Imaging.ImageFormat]::Png)
} finally {
    $font.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
}
[System.IO.File]::WriteAllText((Join-Path $fixtureRoot 'Project notes.md'), '# Generated local test notes')
[System.IO.File]::WriteAllText((Join-Path $fixtureRoot 'Copy A.txt'), 'Generated disposable duplicate fixture.')
[System.IO.File]::WriteAllText((Join-Path $fixtureRoot 'Copy B.txt'), 'Generated disposable duplicate fixture.')
[System.IO.File]::WriteAllText((Join-Path $fixtureRoot 'Report.pdf'), "%PDF-1.4`n% Generated classification fixture, not a complete PDF")
[System.IO.File]::WriteAllText((Join-Path $fixtureRoot 'Design assets.zip'), 'Generated classification fixture, not a complete archive')
Set-Content -LiteralPath (Join-Path $fixtureRoot 'Project notes.md') -Stream Zone.Identifier -Value "[ZoneTransfer]`nZoneId=3`nHostUrl=https://github.com/example/project"
Write-Output $fixtureRoot
