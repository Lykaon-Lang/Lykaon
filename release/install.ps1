# Lykaon Windows installer.
# Copies the binaries + icon to %LOCALAPPDATA%\Lykaon, adds it to the User PATH,
# registers the .lykaon file type + icon (pointing at the installed copy), and
# refreshes Explorer. Run once (double-click install.bat, or run this script).
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dest = Join-Path $env:LOCALAPPDATA "Lykaon"

# 1. Install the binaries + icon to a stable location.
New-Item -ItemType Directory -Force $dest | Out-Null
foreach ($f in @("lykaonc-cli.exe", "lykaon-cli.exe", "lykaon_rt.lib", "lykaon.ico")) {
    $src = Join-Path $here $f
    if (Test-Path $src) {
        Copy-Item -Force $src (Join-Path $dest $f)
    }
}
$icon = Join-Path $dest "lykaon.ico"
if (-not (Test-Path $icon)) {
    Write-Host "lykaon.ico not found next to this script or in $dest." -ForegroundColor Red
    exit 1
}

# 2. Add $dest to the User PATH (once).
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$dest*") {
    $newPath = if ([string]::IsNullOrEmpty($userPath)) { $dest } else { "$userPath;$dest" }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Added $dest to the User PATH."
}

# 3. Register the file type + icon (DefaultIcon points at the installed copy).
reg add "HKCU\Software\Classes\.lykaon" /ve /d "Lykaon.File" /f | Out-Null
reg add "HKCU\Software\Classes\Lykaon.File" /ve /d "Lykaon Source File" /f | Out-Null
reg add "HKCU\Software\Classes\Lykaon.File\DefaultIcon" /ve /d "$icon,0" /f | Out-Null

$cli = Join-Path $dest "lykaonc-cli.exe"
if (Test-Path $cli) {
    $cmd = "`"$cli`" run `"%1`""
    reg add "HKCU\Software\Classes\Lykaon.File\shell\open\command" /ve /d $cmd /f | Out-Null
    Write-Host "Registered .lykaon (open with lykaonc-cli run)."
} else {
    Write-Host "Registered .lykaon file type and icon."
}

# 4. Tell Explorer the associations changed (SHCNE_ASSOCCHANGED).
try {
    Add-Type -Namespace Win32 -Name Shell -MemberDefinition @'
[DllImport("shell32.dll")]
public static extern void SHChangeNotify(int eventId, uint flags, IntPtr item1, IntPtr item2);
'@
    [Win32.Shell]::SHChangeNotify(0x08000000, 0x0000, [IntPtr]::Zero, [IntPtr]::Zero)
} catch {
    try { ie4uinit.exe -show } catch {}
}

Write-Host ""
Write-Host "Installed to $dest"
Write-Host ".lykaon files now show the Lykaon wolf icon in File Explorer."
Write-Host "Open a new terminal to use lykaonc-cli / lykaon-cli from PATH."
Write-Host ""
Read-Host "Press Enter to close"