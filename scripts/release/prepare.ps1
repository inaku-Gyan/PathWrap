[CmdletBinding()]
param(
    [ValidateSet('Validate', 'Package', 'SelfTest')]
    [string]$Mode = 'Validate',
    [string]$Tag,
    [string]$TargetDirectory,
    [string]$OutputDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RepositoryRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
}

function Get-ReleaseVersion {
    param([Parameter(Mandatory)][string]$ReleaseTag)

    $match = [regex]::Match($ReleaseTag, '^v(?<version>(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*))$')
    if (-not $match.Success) {
        throw "Release tag '$ReleaseTag' must be a stable vX.Y.Z tag."
    }

    return $match.Groups['version'].Value
}

function Get-PackageVersion {
    $metadataJson = cargo metadata --locked --no-deps --format-version 1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo metadata failed while reading the package version.'
    }

    $metadata = $metadataJson | ConvertFrom-Json
    $packages = @($metadata.packages | Where-Object { $_.name -ceq 'PathWarp' })
    if ($packages.Count -ne 1) {
        throw "Expected exactly one PathWarp package, found $($packages.Count)."
    }

    return [string]$packages[0].version
}

function Assert-VersionMatches {
    param(
        [Parameter(Mandatory)][string]$ReleaseTag,
        [Parameter(Mandatory)][string]$PackageVersion
    )

    $releaseVersion = Get-ReleaseVersion $ReleaseTag
    if ($PackageVersion -cne $releaseVersion) {
        throw "Release tag '$ReleaseTag' expects package version '$releaseVersion', but Cargo reports '$PackageVersion'."
    }

    return $releaseVersion
}

function Get-ChecksumLine {
    param([Parameter(Mandatory)][string]$ZipPath)

    $hash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    return "$hash  $([IO.Path]::GetFileName($ZipPath))"
}

function Assert-ChecksumFile {
    param(
        [Parameter(Mandatory)][string]$ZipPath,
        [Parameter(Mandatory)][string]$ChecksumPath
    )

    if (-not (Test-Path -LiteralPath $ZipPath -PathType Leaf)) {
        throw "ZIP does not exist: $ZipPath"
    }
    if (-not (Test-Path -LiteralPath $ChecksumPath -PathType Leaf)) {
        throw "Checksum file does not exist: $ChecksumPath"
    }

    $line = (Get-Content -LiteralPath $ChecksumPath -Raw).Trim()
    $parts = $line -split '\s+', 2
    if ($parts.Count -ne 2) {
        throw "Checksum file '$ChecksumPath' must contain a SHA-256 and filename."
    }

    $expectedHash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $actualHash = $parts[0].ToLowerInvariant()
    $expectedName = [IO.Path]::GetFileName($ZipPath)
    $actualName = $parts[1].TrimStart('*')
    if ($actualHash -cne $expectedHash -or $actualName -cne $expectedName) {
        throw "Checksum mismatch for '$expectedName'."
    }
}

function Write-ChecksumFile {
    param([Parameter(Mandatory)][string]$ZipPath)

    $checksumPath = "$ZipPath.sha256"
    Set-Content -LiteralPath $checksumPath -Value (Get-ChecksumLine $ZipPath) -Encoding ascii -NoNewline
    Assert-ChecksumFile -ZipPath $ZipPath -ChecksumPath $checksumPath
    return $checksumPath
}

function New-PortablePackage {
    param(
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$TargetRoot,
        [Parameter(Mandatory)][string]$DestinationRoot
    )

    $repositoryRoot = Get-RepositoryRoot
    New-Item -ItemType Directory -Path $DestinationRoot -Force | Out-Null

    $packageRootName = "PathWarp-v$Version-windows-x64"
    $packageRoot = Join-Path $DestinationRoot $packageRootName
    if (Test-Path -LiteralPath $packageRoot) {
        Remove-Item -LiteralPath $packageRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null

    $binaryPath = Join-Path $TargetRoot 'release\PathWarp.exe'
    if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
        throw "Release binary does not exist: $binaryPath"
    }

    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $packageRoot 'PathWarp.exe') -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'LICENSE') -Destination (Join-Path $packageRoot 'LICENSE') -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'README.md') -Destination (Join-Path $packageRoot 'README.md') -Force

    $zipPath = Join-Path $DestinationRoot "$packageRootName.zip"
    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }
    Compress-Archive -LiteralPath $packageRoot -DestinationPath $zipPath -CompressionLevel Optimal
    $checksumPath = Write-ChecksumFile $zipPath

    [pscustomobject]@{
        PackageRoot = $packageRoot
        ZipPath = $zipPath
        ChecksumPath = $checksumPath
    }
}

function Assert-Throws {
    param(
        [Parameter(Mandatory)][scriptblock]$Action,
        [Parameter(Mandatory)][string]$Name
    )

    $threw = $false
    try {
        & $Action
    } catch {
        $threw = $true
    }
    if (-not $threw) {
        throw "Self-test '$Name' expected an error."
    }
}

function Invoke-SelfTest {
    if ((Get-ReleaseVersion 'v0.1.0') -cne '0.1.0') {
        throw 'Stable tag parsing self-test failed.'
    }
    Assert-Throws -Name 'prerelease tag' -Action { Get-ReleaseVersion 'v0.1.0-rc1' }
    Assert-Throws -Name 'missing v prefix' -Action { Get-ReleaseVersion '0.1.0' }
    Assert-Throws -Name 'version mismatch' -Action { Assert-VersionMatches 'v0.1.0' '0.2.0' }

    $temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) "PathWarp-release-selftest-$([guid]::NewGuid().ToString('N'))"
    try {
        New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null
        $zipPath = Join-Path $temporaryRoot 'sample.zip'
        Set-Content -LiteralPath $zipPath -Value 'self-test' -Encoding ascii
        $checksumPath = Write-ChecksumFile $zipPath
        Set-Content -LiteralPath $checksumPath -Value (('0' * 64) + '  sample.zip') -Encoding ascii -NoNewline
        Assert-Throws -Name 'checksum mismatch' -Action {
            Assert-ChecksumFile -ZipPath $zipPath -ChecksumPath $checksumPath
        }
    } finally {
        if (Test-Path -LiteralPath $temporaryRoot) {
            Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
        }
    }

    Write-Output 'Release preparation self-tests passed.'
}

if ($Mode -eq 'SelfTest') {
    Invoke-SelfTest
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Tag)) {
    throw '-Tag is required for release preparation.'
}

$version = Assert-VersionMatches -ReleaseTag $Tag -PackageVersion (Get-PackageVersion)
if ($Mode -eq 'Validate') {
    Write-Output "Release tag '$Tag' matches package version '$version'."
    exit 0
}

if ([string]::IsNullOrWhiteSpace($TargetDirectory)) {
    throw '-TargetDirectory is required when packaging.'
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    throw '-OutputDirectory is required when packaging.'
}

$package = New-PortablePackage -Version $version -TargetRoot $TargetDirectory -DestinationRoot $OutputDirectory
$package | ConvertTo-Json -Compress
