# Register the Windows Explorer folder command for the current user.
[CmdletBinding()]
param([switch]$Uninstall)

$ErrorActionPreference = 'Stop'
$verb = 'HKCU:\Software\Classes\Directory\shell\BoxBoxFiles'

if ($Uninstall) {
    if (Test-Path -LiteralPath $verb) {
        Remove-Item -LiteralPath $verb -Recurse -Force
    }
    return
}

$app = Get-Item -LiteralPath (Join-Path $PSScriptRoot 'boxbox-files.exe')
if ($app.PSIsContainer) { throw 'boxbox-files.exe must be a file in this folder' }
New-Item -Path $verb -Force | Out-Null
Set-Item -LiteralPath $verb -Value 'Open in BoxBox Files'
Set-ItemProperty -LiteralPath $verb -Name 'Icon' -Value $app.FullName
$command = Join-Path $verb 'command'
New-Item -Path $command -Force | Out-Null
Set-Item -LiteralPath $command -Value ('"' + $app.FullName + '" -- "%1"')
