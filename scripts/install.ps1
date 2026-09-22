# Installs Close My Lid on Windows from the latest GitHub release.
#
#   irm https://raw.githubusercontent.com/krishkalaria12/close-my-lid/main/scripts/install.ps1 | iex
#
# Installs the tray app and the `close-my-lid` CLI into
# %LOCALAPPDATA%\Programs\CloseMyLid, adds that folder to the user PATH, and
# creates a Start menu shortcut for the tray app. No administrator rights needed.
#
# Environment:
#   CLOSE_MY_LID_VERSION       release to install, e.g. v0.5.0 (default: latest)
#   CLOSE_MY_LID_INSTALL_DIR   install folder (default: %LOCALAPPDATA%\Programs\CloseMyLid)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # Invoke-WebRequest is many times slower with the progress bar.

$Repo = 'krishkalaria12/close-my-lid'
$InstallDir = if ($env:CLOSE_MY_LID_INSTALL_DIR) { $env:CLOSE_MY_LID_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\CloseMyLid' }

[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'Close My Lid needs 64-bit Windows.'
}

if ($env:CLOSE_MY_LID_VERSION) {
    $Tag = if ($env:CLOSE_MY_LID_VERSION.StartsWith('v')) { $env:CLOSE_MY_LID_VERSION } else { "v$($env:CLOSE_MY_LID_VERSION)" }
} else {
    $Tag = (Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ 'User-Agent' = 'close-my-lid-installer' }).tag_name
}

$Name = "Close-My-Lid-$Tag-windows-x86_64"
$Url = "https://github.com/$Repo/releases/download/$Tag/$Name.zip"
$Tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $Tmp | Out-Null

try {
    Write-Host "Downloading $Name.zip"
    Invoke-WebRequest -Uri $Url -OutFile (Join-Path $Tmp "$Name.zip") -UseBasicParsing
    Expand-Archive -Path (Join-Path $Tmp "$Name.zip") -DestinationPath $Tmp -Force

    $Source = Join-Path $Tmp $Name
    if (-not (Test-Path (Join-Path $Source 'close-my-lid-gui.exe'))) {
        throw "$Name.zip does not contain close-my-lid-gui.exe"
    }

    # A running tray app locks its executable. Release any hold through the
    # CLI first, so killing the app cannot leave the lid action changed.
    $Running = Get-Process -Name 'close-my-lid-gui' -ErrorAction SilentlyContinue
    if ($Running) {
        $OldCli = Join-Path $InstallDir 'close-my-lid.exe'
        if (Test-Path $OldCli) { try { & $OldCli disable *> $null } catch { } }
        Write-Host 'Stopping the running tray app'
        $Running | Stop-Process -Force
        Start-Sleep -Seconds 1
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path (Join-Path $Source '*') -Destination $InstallDir -Recurse -Force

    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $Entries = if ($UserPath) { $UserPath.Split(';') } else { @() }
    if ($Entries -notcontains $InstallDir) {
        [Environment]::SetEnvironmentVariable('Path', (($Entries + $InstallDir) | Where-Object { $_ }) -join ';', 'User')
        $env:Path = "$env:Path;$InstallDir"
        $PathChanged = $true
    }

    $StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
    $Shell = New-Object -ComObject WScript.Shell
    $Shortcut = $Shell.CreateShortcut((Join-Path $StartMenu 'Close My Lid.lnk'))
    $Shortcut.TargetPath = Join-Path $InstallDir 'close-my-lid-gui.exe'
    $Shortcut.WorkingDirectory = $InstallDir
    $Shortcut.Save()

    Write-Host ''
    Write-Host "Installed Close My Lid $Tag to $InstallDir"
    Write-Host 'Start it from the Start menu (Close My Lid), or run: close-my-lid enable --for 2h'
    if ($PathChanged) {
        Write-Host 'Open a new terminal to pick up close-my-lid on your PATH.'
    }
    if ($Running) {
        Start-Process -FilePath (Join-Path $InstallDir 'close-my-lid-gui.exe')
    }
} finally {
    Remove-Item -Path $Tmp -Recurse -Force -ErrorAction SilentlyContinue
}
