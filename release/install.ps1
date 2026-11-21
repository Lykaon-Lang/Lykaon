# Lykaon Windows installer: register the .lykaon file type + icon.
# Run this once (double-click in Explorer, or: powershell -ExecutionPolicy Bypass -File install.ps1)
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$icon = Join-Path $here "lykaon.ico"

if (-not (Test-Path $icon)) {
    Write-Host "lykaon.ico not found next to this script." -ForegroundColor Red
    exit 1
}

# Register the file type for the current user (no admin needed).
reg add "HKCU\Software\Classes\.lykaon" /ve /d "Lykaon.File" /f | Out-Null
reg add "HKCU\Software\Classes\Lykaon.File" /ve /d "Lykaon Source File" /f | Out-Null
reg add "HKCU\Software\Classes\Lykaon.File\DefaultIcon" /ve /d "$icon,0" /f | Out-Null

$cli = Join-Path $here "lykaonc-cli.exe"
if (Test-Path $cli) {
    $cmd = "`"$cli`" run `"%1`""
    reg add "HKCU\Software\Classes\Lykaon.File\shell\open\command" /ve /d $cmd /f | Out-Null
    Write-Host "Registered .lykaon (open with lykaonc-cli run)."
} else {
    Write-Host "Registered .lykaon file type and icon."
}

# Refresh the Explorer icon cache.
try { ie4uinit.exe -show } catch {}

Write-Host ""
Write-Host "Done. .lykaon files now show the Lykaon wolf icon in File Explorer."
Write-Host "To use the editor features, install editors/vscode/lykaon-0.1.0.vsix (or publish it)."
Write-Host ""
Read-Host "Press Enter to close"