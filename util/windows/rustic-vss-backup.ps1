#Requires -Version 5.1
<#
.SYNOPSIS
Back up one local Windows path from a temporary VSS shadow copy with rustic.

.DESCRIPTION
Creates a ClientAccessible Win32_ShadowCopy for the source drive, invokes
rustic with the shadow copy's DeviceObject path, and removes the shadow copy
afterwards. See README.md in this directory for setup and limitations.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Source,

    [string]$RusticExe = "rustic",

    # Passed to `rustic backup` after the source and --as-path arguments.
    [string[]]$RusticArgument = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$principal = [Security.Principal.WindowsPrincipal]::new(
    [Security.Principal.WindowsIdentity]::GetCurrent()
)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Run this script from an elevated PowerShell session; Windows VSS creation requires Administrator privileges."
}

$sourceItem = Get-Item -LiteralPath $Source -Force
$sourcePath = [System.IO.Path]::GetFullPath($sourceItem.FullName)
$sourceDrive = [System.IO.Path]::GetPathRoot($sourcePath)

# This example deliberately limits itself to a local drive-letter volume. A
# VSS shadow copy cannot be made from a UNC path, and mounted volumes need
# their own volume-root handling.
if ([string]::IsNullOrWhiteSpace($sourceDrive) -or $sourceDrive -notmatch '^[A-Za-z]:\\$') {
    throw "Source must be on a local drive-letter volume, for example C:\Users\alice."
}

$relativeSource = $sourcePath.Substring($sourceDrive.Length).TrimStart('\')
$rustic = Get-Command -Name $RusticExe -CommandType Application -ErrorAction Stop

$shadow = $null
$exitCode = 1

try {
    $result = Invoke-CimMethod -ClassName Win32_ShadowCopy -MethodName Create -Arguments @{
        Volume = $sourceDrive
        Context = "ClientAccessible"
    }

    if ($result.ReturnValue -ne 0 -or [string]::IsNullOrWhiteSpace($result.ShadowID)) {
        throw "Windows VSS creation failed with return value $($result.ReturnValue)."
    }

    $shadow = Get-CimInstance -ClassName Win32_ShadowCopy -Filter "ID = '$($result.ShadowID)'"
    if ($null -eq $shadow -or [string]::IsNullOrWhiteSpace($shadow.DeviceObject)) {
        throw "Windows created the VSS snapshot but did not expose its device path."
    }

    $shadowSource = "$($shadow.DeviceObject)\$relativeSource"
    Write-Verbose "Backing up VSS source '$shadowSource' as '$sourcePath'."

    & $rustic.Path backup $shadowSource --as-path $sourcePath @RusticArgument
    $exitCode = $LASTEXITCODE
}
finally {
    if ($null -ne $shadow) {
        try {
            Remove-CimInstance -InputObject $shadow -ErrorAction Stop
        }
        catch {
            [Console]::Error.WriteLine("Unable to remove VSS shadow copy '$($shadow.ID)': $($_.Exception.Message)")
            if ($exitCode -eq 0) {
                $exitCode = 1
            }
        }
    }
}

exit $exitCode
