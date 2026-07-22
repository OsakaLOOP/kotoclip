param(
  [string]$SourcePath = "src/version/changelog.source.json",
  [string]$OutputPath = "src/version/changelog.json",
  [string]$RefreshVersion = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$sourceFile = if ([System.IO.Path]::IsPathRooted($SourcePath)) { $SourcePath } else { Join-Path $root $SourcePath }
$outputFile = if ([System.IO.Path]::IsPathRooted($OutputPath)) { $OutputPath } else { Join-Path $root $OutputPath }

$existingReleases = @{}
if (Test-Path $outputFile) {
  $existingDocument = Get-Content -Raw -Encoding UTF8 $outputFile | ConvertFrom-Json
  foreach ($existingRelease in $existingDocument.releases) {
    $existingReleases[$existingRelease.version] = $existingRelease
  }
}

$document = Get-Content -Raw -Encoding UTF8 $sourceFile | ConvertFrom-Json
foreach ($release in $document.releases) {
  if ($release.source.kind -eq "commit" -and $release.source.ref -eq "HEAD") {
    $existingRelease = $existingReleases[$release.version]
    $existingRef = $existingRelease.source.ref
    $canReuseExistingRef = $RefreshVersion -ne $release.version `
      -and $null -ne $existingRelease `
      -and $existingRelease.source.kind -eq "commit" `
      -and $existingRef -match "^[0-9a-fA-F]{40}$"
    $release.source.ref = if ($canReuseExistingRef) {
      $existingRef
    } else {
      (git -C $root rev-parse HEAD).Trim()
    }
  }
  if ($release.source.kind -eq "commit") {
    $url = "$($document.repositoryUrl)/commit/$($release.source.ref)"
  } elseif ($release.source.kind -eq "release") {
    $url = "$($document.repositoryUrl)/releases/tag/v$($release.version)"
  }
  $release.source | Add-Member -NotePropertyName url -NotePropertyValue $url -Force
}

$document | Add-Member -NotePropertyName latestVersion -NotePropertyValue $document.releases[0].version -Force
$document | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 $outputFile
