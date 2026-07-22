param(
  [string]$SourcePath = "src/version/changelog.source.json",
  [string]$OutputPath = "src/version/changelog.json"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$sourceFile = Join-Path $root $SourcePath
$outputFile = Join-Path $root $OutputPath

$document = Get-Content -Raw -Encoding UTF8 $sourceFile | ConvertFrom-Json
foreach ($release in $document.releases) {
  if ($release.source.kind -eq "commit" -and $release.source.ref -eq "HEAD") {
    $release.source.ref = (git -C $root rev-parse HEAD).Trim()
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
