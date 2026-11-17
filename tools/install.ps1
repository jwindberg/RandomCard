# Build RandomCard and install it for this user, into %USERPROFILE%\.cargo\bin.
# Rerun to upgrade. Works from any folder:
#   powershell -ExecutionPolicy Bypass -File tools\install.ps1
# First it offers to install what building needs: Microsoft's C++ build tools
# (Rust links with them, and their resource compiler gives the program its
# icon) and Rust itself, with rustup.
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot

function Ask($question) {
    # Yes unless answered no; no when there's no one to ask.
    if ([Console]::IsInputRedirected) { return $false }
    $answer = Read-Host "$question [Y/n]"
    return -not ($answer -match '^[Nn]')
}

function Fail($message) {
    [Console]::Error.WriteLine($message)
    exit 1
}

$arm = $env:PROCESSOR_ARCHITECTURE -eq 'ARM64'
$vcTools = if ($arm) { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
$triple = if ($arm) { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }

# Microsoft's C++ build tools, found with the Visual Studio Installer's vswhere.
function Have-BuildTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { return $false }
    $found = & $vswhere -latest -products * -requires $vcTools -property installationPath
    return [bool]$found
}

if (-not (Have-BuildTools)) {
    Write-Host "Building needs Microsoft's C++ build tools, which aren't installed."
    $winget = Get-Command winget -ErrorAction SilentlyContinue
    if ($winget -and (Ask 'Install them now (with winget; this takes a while)?')) {
        $add = '--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
        if ($arm) { $add += " --add $vcTools" }
        winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements --override "--wait --passive $add"
        if (-not (Have-BuildTools)) { Fail 'The build tools did not install; run tools\install.ps1 again once they have.' }
    } else {
        Fail 'Install "Desktop development with C++" from https://visualstudio.microsoft.com/visual-cpp-build-tools/ first.'
    }
}

# rustup puts cargo in %USERPROFILE%\.cargo\bin, which this shell may not
# have on its PATH yet.
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$cargoBin = Join-Path $cargoHome 'bin'
if (-not (Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path (Join-Path $cargoBin 'cargo.exe'))) {
    $env:Path = "$cargoBin;$env:Path"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust isn't installed."
    if (Ask 'Install it now with rustup (from https://rustup.rs)?') {
        $init = Join-Path $env:TEMP 'rustup-init.exe'
        Invoke-WebRequest -UseBasicParsing -Uri "https://static.rust-lang.org/rustup/dist/$triple/rustup-init.exe" -OutFile $init
        & $init -y
        if ($LASTEXITCODE -ne 0) { Fail 'rustup did not finish installing Rust.' }
        Remove-Item $init -ErrorAction SilentlyContinue
        $env:Path = "$cargoBin;$env:Path"
    } else {
        Fail 'Install Rust from https://rustup.rs first.'
    }
}

cargo install --locked --path $Root
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host 'RandomCard is installed. tools\install_desktop.ps1 puts it in the Start menu.'
