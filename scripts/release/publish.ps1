[CmdletBinding()]
param(
    [ValidateSet('Publish', 'SelfTest')]
    [string]$Mode = 'Publish',
    [string]$Tag,
    [string]$Repository = $env:GITHUB_REPOSITORY,
    [string]$Commit = $env:GITHUB_SHA,
    [string]$ArtifactDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-StableTag {
    param([Parameter(Mandatory)][string]$ReleaseTag)

    if (-not [regex]::IsMatch($ReleaseTag, '^v(?<version>(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*))$')) {
        throw "Release tag '$ReleaseTag' must be a stable vX.Y.Z tag."
    }
}

function Get-PublishAction {
    param(
        [Parameter(Mandatory)][string]$ReleaseTag,
        [Parameter(Mandatory)][string]$TagCommit,
        [Parameter(Mandatory)][string]$ExpectedCommit,
        [AllowNull()][object]$ExistingRelease
    )

    if ($TagCommit.ToLowerInvariant() -cne $ExpectedCommit.ToLowerInvariant()) {
        throw "Tag '$ReleaseTag' points to '$TagCommit', but this run is for '$ExpectedCommit'."
    }

    if ($null -eq $ExistingRelease) {
        return 'CreateDraft'
    }

    if ([string]$ExistingRelease.tagName -cne $ReleaseTag) {
        throw "Existing release tag '$($ExistingRelease.tagName)' does not match '$ReleaseTag'."
    }
    if (-not [bool]$ExistingRelease.isDraft) {
        throw "Release '$ReleaseTag' is already published; refusing to overwrite public assets."
    }

    return 'ReuseDraft'
}

function Invoke-GhReleaseView {
    param(
        [Parameter(Mandatory)][string]$ReleaseTag,
        [Parameter(Mandatory)][string]$Repo
    )

    $output = gh release view $ReleaseTag --repo $Repo --json id,isDraft,tagName,targetCommitish,publishedAt 2>&1 | Out-String
    $exitCode = $LASTEXITCODE
    if ($exitCode -eq 0) {
        return $output | ConvertFrom-Json
    }

    if ($output -match '(?i)release not found|http 404|not found') {
        return $null
    }

    throw "Unable to inspect release '$ReleaseTag': $($output.Trim())"
}

function Get-TagCommit {
    param(
        [Parameter(Mandatory)][string]$ReleaseTag,
        [Parameter(Mandatory)][string]$Repo
    )

    $output = gh api "repos/$Repo/commits/$ReleaseTag" --jq .sha 2>&1 | Out-String
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        throw "Unable to resolve tag '$ReleaseTag': $($output.Trim())"
    }

    $commit = $output.Trim()
    if ([string]::IsNullOrWhiteSpace($commit)) {
        throw "GitHub returned no commit for tag '$ReleaseTag'."
    }

    return $commit
}

function Invoke-GhCommand {
    param(
        [Parameter(Mandatory)][string]$Description,
        [Parameter(Mandatory)][scriptblock]$Command
    )

    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

function Invoke-Publish {
    param(
        [Parameter(Mandatory)][string]$ReleaseTag,
        [Parameter(Mandatory)][string]$Repo,
        [Parameter(Mandatory)][string]$ExpectedCommit,
        [Parameter(Mandatory)][string]$Artifacts
    )

    Assert-StableTag $ReleaseTag
    if ([string]::IsNullOrWhiteSpace($Repo)) {
        throw 'Repository is required when publishing a release.'
    }
    if ([string]::IsNullOrWhiteSpace($ExpectedCommit)) {
        throw 'Commit is required when publishing a release.'
    }
    if ([string]::IsNullOrWhiteSpace($Artifacts)) {
        throw 'ArtifactDirectory is required when publishing a release.'
    }

    $zipName = "PathWarp-$ReleaseTag-windows-x64.zip"
    $checksumName = "$zipName.sha256"
    $zipPath = Join-Path $Artifacts $zipName
    $checksumPath = Join-Path $Artifacts $checksumName
    if (-not (Test-Path -LiteralPath $zipPath -PathType Leaf)) {
        throw "Candidate ZIP does not exist: $zipPath"
    }
    if (-not (Test-Path -LiteralPath $checksumPath -PathType Leaf)) {
        throw "Candidate checksum does not exist: $checksumPath"
    }

    $prepareScript = Join-Path $PSScriptRoot 'prepare.ps1'
    & pwsh -NoProfile -File $prepareScript -Mode Verify -ZipPath $zipPath -ChecksumPath $checksumPath
    if ($LASTEXITCODE -ne 0) {
        throw 'Candidate checksum verification failed.'
    }

    $tagCommit = Get-TagCommit -ReleaseTag $ReleaseTag -Repo $Repo
    $existingRelease = Invoke-GhReleaseView -ReleaseTag $ReleaseTag -Repo $Repo
    $action = Get-PublishAction -ReleaseTag $ReleaseTag -TagCommit $tagCommit -ExpectedCommit $ExpectedCommit -ExistingRelease $existingRelease

    if ($action -eq 'CreateDraft') {
        Invoke-GhCommand -Description 'Creating draft release' -Command {
            gh release create $ReleaseTag --repo $Repo --draft --generate-notes --verify-tag
        }
    }

    Invoke-GhCommand -Description 'Uploading release assets' -Command {
        gh release upload $ReleaseTag $zipPath $checksumPath --repo $Repo --clobber
    }
    Invoke-GhCommand -Description 'Publishing release' -Command {
        gh release edit $ReleaseTag --repo $Repo --draft=false
    }

    $serverUrl = if ([string]::IsNullOrWhiteSpace($env:GITHUB_SERVER_URL)) { 'https://github.com' } else { $env:GITHUB_SERVER_URL.TrimEnd('/') }
    $releaseUrl = "$serverUrl/$Repo/releases/tag/$ReleaseTag"
    if (-not [string]::IsNullOrWhiteSpace($env:GITHUB_STEP_SUMMARY)) {
        @(
            '## GitHub Release'
            "- Commit SHA: $ExpectedCommit"
            "- Tag: $ReleaseTag"
            "- Action: $action"
            "- Release: $releaseUrl"
        ) | Out-File -FilePath $env:GITHUB_STEP_SUMMARY -Append -Encoding utf8
    }

    [pscustomobject]@{
        Action = $action
        Tag = $ReleaseTag
        Commit = $ExpectedCommit
        ReleaseUrl = $releaseUrl
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
    Assert-StableTag 'v0.1.0'
    Assert-Throws -Name 'prerelease tag' -Action { Assert-StableTag 'v0.1.0-rc1' }
    Assert-Throws -Name 'missing v prefix' -Action { Assert-StableTag '0.1.0' }

    $commit = 'a' * 40
    if ((Get-PublishAction -ReleaseTag 'v0.1.0' -TagCommit $commit -ExpectedCommit $commit -ExistingRelease $null) -cne 'CreateDraft') {
        throw 'Self-test successful publish decision failed.'
    }

    $draft = [pscustomobject]@{ tagName = 'v0.1.0'; isDraft = $true }
    if ((Get-PublishAction -ReleaseTag 'v0.1.0' -TagCommit $commit -ExpectedCommit $commit -ExistingRelease $draft) -cne 'ReuseDraft') {
        throw 'Self-test draft rerun decision failed.'
    }

    $published = [pscustomobject]@{ tagName = 'v0.1.0'; isDraft = $false }
    Assert-Throws -Name 'published release conflict' -Action {
        Get-PublishAction -ReleaseTag 'v0.1.0' -TagCommit $commit -ExpectedCommit $commit -ExistingRelease $published
    }
    Assert-Throws -Name 'tag conflict' -Action {
        Get-PublishAction -ReleaseTag 'v0.1.0' -TagCommit ('b' * 40) -ExpectedCommit $commit -ExistingRelease $null
    }

    Write-Output 'Release publication self-tests passed.'
}

if ($Mode -eq 'SelfTest') {
    Invoke-SelfTest
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Tag)) {
    throw '-Tag is required when publishing a release.'
}

Invoke-Publish -ReleaseTag $Tag -Repo $Repository -ExpectedCommit $Commit -Artifacts $ArtifactDirectory | ConvertTo-Json -Compress
