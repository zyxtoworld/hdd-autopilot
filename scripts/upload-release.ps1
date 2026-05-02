param(
  [Parameter(Position = 0)]
  [string]$Tag,

  [string]$Repo,
  [string]$Dist,
  [string]$Title,
  [string]$Notes,

  [switch]$Draft,
  [switch]$Prerelease,
  [switch]$AllowDegraded,
  [switch]$NoClobber,
  [switch]$Help
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"

function Show-Usage {
  @"
Usage:
  powershell -NoLogo -ExecutionPolicy Bypass -File scripts/upload-release.ps1 <tag> [options]
  pwsh -NoLogo -File scripts/upload-release.ps1 <tag> [options]

Options:
  -Repo <owner/name>       Upload to a specific GitHub repository.
  -Dist <path>             Package directory. Defaults to repo-root/dist.
  -Title <text>            Release title. Defaults to the tag.
  -Notes <text>            Release notes. Defaults to generated notes.
  -Draft                   Create the release as a draft when it does not exist.
  -Prerelease              Create the release as a prerelease when it does not exist.
  -AllowDegraded           Allow assets whose .status is built_degraded.
  -NoClobber               Do not overwrite existing release assets.
  -Help                    Show this help.

The script scans dist/hdd-autopilot-*.status and uploads matching package files
only when the status is built, unless -AllowDegraded is set.
"@
}

if ($Help) {
  Show-Usage
  exit 0
}

if ([string]::IsNullOrWhiteSpace($Tag)) {
  Show-Usage
  exit 2
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptRoot
if ([string]::IsNullOrWhiteSpace($Dist)) {
  $Dist = Join-Path $repoRoot "dist"
}
if ([string]::IsNullOrWhiteSpace($Title)) {
  $Title = $Tag
}

$gh = Get-Command gh -ErrorAction SilentlyContinue
if ($null -eq $gh) {
  throw "GitHub CLI 'gh' is required. Install it and run 'gh auth login' first."
}

& $gh.Source auth status *> $null
if ($LASTEXITCODE -ne 0) {
  throw "GitHub CLI is not authenticated. Run 'gh auth login' first."
}

$distPath = Resolve-Path -LiteralPath $Dist -ErrorAction Stop
$statusFiles = @(Get-ChildItem -LiteralPath $distPath -Filter "hdd-autopilot-*.status" -File)
if ($statusFiles.Count -eq 0) {
  throw "No package status files found in '$distPath'. Run a build script first."
}

$assets = New-Object System.Collections.Generic.List[string]
$blocked = New-Object System.Collections.Generic.List[string]
$allowedBasePattern = "^hdd-autopilot-(x86_64-pc-windows-msvc|x86_64-apple-darwin|aarch64-apple-darwin|x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu)(-[A-Za-z0-9][A-Za-z0-9._-]*)?$"

foreach ($statusFile in $statusFiles) {
  $status = [System.IO.File]::ReadAllText($statusFile.FullName).Trim()
  $baseName = $statusFile.Name.Substring(0, $statusFile.Name.Length - ".status".Length)
  if ($baseName -notmatch $allowedBasePattern) {
    $blocked.Add("$($statusFile.Name)=unsupported-name")
    continue
  }

  $basePath = Join-Path $statusFile.DirectoryName $baseName
  $assetPath = $null

  if (Test-Path -LiteralPath $basePath -PathType Leaf) {
    $assetPath = (Resolve-Path -LiteralPath $basePath).Path
  } elseif (Test-Path -LiteralPath "$basePath.exe" -PathType Leaf) {
    $assetPath = (Resolve-Path -LiteralPath "$basePath.exe").Path
  }

  $isAllowed = $status -eq "built" -or ($AllowDegraded -and $status -eq "built_degraded")
  if ($isAllowed) {
    if ($null -eq $assetPath) {
      throw "Status '$($statusFile.Name)' is '$status', but the matching package file is missing."
    }
    $assets.Add($assetPath)
  } else {
    $blocked.Add("$($statusFile.Name)=$status")
  }
}

if ($blocked.Count -gt 0) {
  $message = "Refusing to upload because non-built package status exists: " + ($blocked -join ", ")
  if (-not $AllowDegraded) {
    $message += ". Use -AllowDegraded only if you intentionally want degraded packages."
  }
  throw $message
}

if ($assets.Count -eq 0) {
  throw "No uploadable package assets found in '$distPath'."
}

$repoArgs = @()
if (-not [string]::IsNullOrWhiteSpace($Repo)) {
  $repoArgs += @("--repo", $Repo)
}

& $gh.Source release view $Tag @repoArgs *> $null
$releaseExists = $LASTEXITCODE -eq 0
if (-not $releaseExists) {
  $createArgs = @("release", "create", $Tag) + $repoArgs + @("--title", $Title)
  if ([string]::IsNullOrWhiteSpace($Notes)) {
    $createArgs += "--generate-notes"
  } else {
    $createArgs += @("--notes", $Notes)
  }
  if ($Draft) {
    $createArgs += "--draft"
  }
  if ($Prerelease) {
    $createArgs += "--prerelease"
  }

  & $gh.Source @createArgs
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to create GitHub Release '$Tag'."
  }
}

$uploadArgs = @("release", "upload", $Tag) + $repoArgs + $assets.ToArray()
if (-not $NoClobber) {
  $uploadArgs += "--clobber"
}

& $gh.Source @uploadArgs
if ($LASTEXITCODE -ne 0) {
  throw "Failed to upload package assets to GitHub Release '$Tag'."
}

Write-Host "Uploaded $($assets.Count) asset(s) to GitHub Release ${Tag}:"
foreach ($asset in $assets) {
  Write-Host "  $(Split-Path -Leaf $asset)"
}
