# Build a portable Windows x64 archive from the current checkout.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

if (![Environment]::Is64BitProcess -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
    throw 'Windows desktop packaging requires a Windows x64 shell'
}

$stage = $null
Push-Location (Join-Path $PSScriptRoot '..')
try {
    $version = (Get-Content -Raw VERSION).Trim()
    if ($version -cnotmatch '^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$') {
        throw 'Invalid VERSION'
    }
    $tag = $env:RELEASE_TAG
    if ($tag) {
        if ($tag -cne "v$version") { throw 'Release tag does not match VERSION' }
        if (git status --porcelain) { throw 'Official releases require a clean checkout' }
    }
    $source = git rev-parse HEAD

    cargo build --locked --release --manifest-path desktop/Cargo.toml
    $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'desktop/target' }
    $executable = Join-Path $targetDir 'release/boxbox-gpui.exe'
    # The ZIP ships no Visual C++ runtime; the root .cargo/config.toml links it in.
    $imports = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes((Resolve-Path $executable).ProviderPath))
    foreach ($runtime in 'VCRUNTIME140.dll', 'MSVCP140.dll') {
        if ($imports.Contains($runtime, [StringComparison]::OrdinalIgnoreCase)) {
            throw "boxbox-gpui.exe needs $runtime, which the portable ZIP does not include"
        }
    }
    $output = New-Item -Path dist/release -ItemType Directory -Force
    $stage = New-Item -Path $output.FullName -Name (".stage-" + [guid]::NewGuid().ToString('N')) -ItemType Directory
    $name = "boxbox-desktop-v$version-windows-amd64"
    $bundle = New-Item -Path $stage.FullName -Name $name -ItemType Directory
    Copy-Item -LiteralPath $executable -Destination (Join-Path $bundle.FullName 'boxbox.exe')
    Copy-Item -LiteralPath LICENSE -Destination $bundle.FullName
    Copy-Item -LiteralPath desktop/packaging/windows-explorer.ps1 -Destination $bundle.FullName
    @"
BoxBox Files v$version for Windows x64
Source: $source

Extract the ZIP and launch boxbox.exe.
Optionally run windows-explorer.ps1 to add Open in BoxBox Files to folder menus.
See https://boxbox.radhey.dev/docs/desktop/#windows for system requirements.
"@ | Set-Content -Encoding utf8 (Join-Path $bundle.FullName 'README.txt')

    # Compress the directory so extracting the ZIP puts the files together.
    $stagedZip = Join-Path $stage.FullName "$name.zip"
    Compress-Archive -LiteralPath $bundle.FullName -DestinationPath $stagedZip
    $destination = Join-Path $output.FullName "$name.zip"
    Move-Item -LiteralPath $stagedZip -Destination $destination -Force
    Write-Output $destination
} finally {
    if ($stage -and $stage.Parent.FullName -eq $output.FullName) {
        Remove-Item -LiteralPath $stage.FullName -Recurse -Force
    }
    Pop-Location
}
