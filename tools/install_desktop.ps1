# Puts RandomCard in the Start menu, for this user: a shortcut to the program
# tools\install.ps1 installed, with the icon built into it. Rerun it after
# installing a new version if the program has moved.
#   powershell -ExecutionPolicy Bypass -File tools\install_desktop.ps1
$ErrorActionPreference = 'Stop'

$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$found = Get-Command randomcard -ErrorAction SilentlyContinue
$bin = if ($found) { $found.Source } else { Join-Path $cargoHome 'bin\randomcard.exe' }
if (-not (Test-Path $bin)) {
    [Console]::Error.WriteLine("randomcard isn't installed yet; run tools\install.ps1 first.")
    exit 1
}

$link = Join-Path ([Environment]::GetFolderPath('Programs')) 'RandomCard.lnk'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($link)
$shortcut.TargetPath = $bin
$shortcut.WorkingDirectory = $env:USERPROFILE
$shortcut.IconLocation = "$bin,0"
$shortcut.Description = 'RandomCard: HyperCard, recreated in Rust'
$shortcut.Save()
Write-Host "RandomCard is in the Start menu, running $bin."
