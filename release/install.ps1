# Lykaon Windows installer.
# Copies the binaries + icon to %LOCALAPPDATA%\Lykaon, adds it to the User PATH,
# registers the .lykaon file type + icon (pointing at the installed copy),
# installs the VS Code extension when VS Code is present, and refreshes Explorer.
# Run once: double-click install.bat (or right-click install.ps1 > Run with PowerShell).
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dest = Join-Path $env:LOCALAPPDATA "Lykaon"

# 1. Install the binaries + icon to a stable location.
New-Item -ItemType Directory -Force $dest | Out-Null
# A running REPL or the editor's language server keeps the .exe files open, so
# the copy would fail. Close them first; VS Code restarts the language server
# on its own when it next needs it.
$running = @(Get-Process -Name "lykaonc-cli", "lykaon-cli" -ErrorAction SilentlyContinue)
if ($running.Count -gt 0) {
    Write-Host "Closing $($running.Count) running Lykaon process(es) so the files can be replaced ..."
    $running | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 700
}
foreach ($f in @("lykaonc-cli.exe", "lykaon-cli.exe", "lykaon_rt.lib", "lykaon.ico")) {
    $src = Join-Path $here $f
    if (-not (Test-Path $src)) { continue }
    $dst = Join-Path $dest $f
    $copied = $false
    for ($i = 1; $i -le 5; $i++) {
        try {
            Copy-Item -Force $src $dst -ErrorAction Stop
            $copied = $true
            break
        } catch {
            Start-Sleep -Milliseconds 700
        }
    }
    if (-not $copied) {
        Write-Host "Could not replace $f - something is still using it." -ForegroundColor Yellow
        Write-Host "Close VS Code and every Lykaon window, then run install.bat again." -ForegroundColor Yellow
        exit 1
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
# Use the registry provider, not reg.exe: reg.exe strips the quotes that paths
# and "%1" need when a path contains spaces.
New-Item -Path 'HKCU:\Software\Classes\.lykaon' -Force | Out-Null
Set-Item -Path 'HKCU:\Software\Classes\.lykaon' -Value 'Lykaon.File'
New-Item -Path 'HKCU:\Software\Classes\Lykaon.File' -Force | Out-Null
Set-Item -Path 'HKCU:\Software\Classes\Lykaon.File' -Value 'Lykaon Source File'
New-Item -Path 'HKCU:\Software\Classes\Lykaon.File\DefaultIcon' -Force | Out-Null
Set-Item -Path 'HKCU:\Software\Classes\Lykaon.File\DefaultIcon' -Value "`"$icon`",0"

$cli = Join-Path $dest "lykaonc-cli.exe"
if (Test-Path $cli) {
    New-Item -Path 'HKCU:\Software\Classes\Lykaon.File\shell\open\command' -Force | Out-Null
    Set-Item -Path 'HKCU:\Software\Classes\Lykaon.File\shell\open\command' -Value "`"$cli`" run `"%1`""
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

# 5. Install the VS Code extension (only if VS Code is installed).
$vsix = Get-ChildItem -Path $here -Filter "lykaon-*.vsix" -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1
$vscodeDone = "skipped (no lykaon-*.vsix next to this script)"
if ($vsix) {
    $codeCmd = $null
    $found = Get-Command code -ErrorAction SilentlyContinue
    if ($found) { $codeCmd = $found.Source }
    if (-not $codeCmd) {
        $candidates = @(
            (Join-Path $env:LOCALAPPDATA "Programs\Microsoft VS Code\bin\code.cmd"),
            (Join-Path $env:ProgramFiles "Microsoft VS Code\bin\code.cmd"),
            (Join-Path ${env:ProgramFiles(x86)} "Microsoft VS Code\bin\code.cmd")
        )
        foreach ($p in $candidates) {
            if ($p -and (Test-Path $p)) { $codeCmd = $p; break }
        }
    }
    if ($codeCmd) {
        Write-Host "Installing the VS Code extension $($vsix.Name) ..."
        # `code` prints a Node deprecation warning on stderr; keep it off the
        # screen without letting the strict error mode abort the install.
        $wasStrict = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        & $codeCmd --install-extension $vsix.FullName --force 2>&1 | Out-Null
        $listed = & $codeCmd --list-extensions 2>$null
        $ErrorActionPreference = $wasStrict
        if ($listed -contains "Lykaon-Lang.lykaon") {
            $vscodeDone = "installed ($($vsix.Name)) - reload any open VS Code window"
        } else {
            $vscodeDone = "FAILED - run this yourself: code --install-extension `"$($vsix.FullName)`""
        }
    } else {
        $vscodeDone = "VS Code not found - later run: code --install-extension `"$($vsix.FullName)`""
    }
}

Write-Host ""
Write-Host "Done."
Write-Host "  Binaries:  $dest"
Write-Host "  VS Code:   $vscodeDone"
Write-Host "  File icon: .lykaon files now show the Lykaon wolf in File Explorer"
Write-Host ""
Write-Host "Open a NEW terminal, then try:"
Write-Host "  lykaonc-cli run hello.lykaon"
Write-Host "  lykaonc-cli build fast hello.lykaon hello.exe"
Write-Host ""
Read-Host "Press Enter to close"