param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("dev", "insider")]
    [string]$Channel
)

$ErrorActionPreference = "Stop"
$workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $workspace
$exitCode = 0
$target = Join-Path $workspace "target"
$targetLimitBytes = 4GB

function Get-DirectoryBytes([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) {
        return [int64]0
    }
    return [int64]((Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum)
}

function Clear-TargetIfOverLimit {
    if (-not (Test-Path -LiteralPath $target)) {
        return
    }

    $targetBytes = Get-DirectoryBytes $target
    if ($targetBytes -le $targetLimitBytes) {
        Write-Output ("target size {0:N2} GiB; limit 4 GiB." -f ($targetBytes / 1GB))
        return
    }

    $buildProcesses = Get-Process -Name cargo, rustc, tauri-app -ErrorAction SilentlyContinue
    if ($buildProcesses) {
        throw "target exceeds 4 GiB while a build process is running; cleanup aborted."
    }

    $workspaceRoot = (Resolve-Path $workspace).Path.TrimEnd("\")
    $targetPath = (Resolve-Path $target).Path.TrimEnd("\")
    if (-not $targetPath.StartsWith($workspaceRoot + "\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing to clean a target path outside the workspace."
    }

    Remove-Item -LiteralPath $targetPath -Recurse -Force
    Write-Output ("cleaned target at {0:N2} GiB before {1} build." -f ($targetBytes / 1GB), $Channel)
}

function Create-InsiderPackage {
    $packageRoot = Join-Path $workspace "packages"
    $packageName = "Kotoclip-insider-portable-win64"
    $packageDir = Join-Path $packageRoot $packageName
    $archive = Join-Path $packageRoot ($packageName + ".zip")
    $workspaceRoot = (Resolve-Path $workspace).Path.TrimEnd("\")

    foreach ($path in @($packageRoot, $packageDir, $archive)) {
        $fullPath = [System.IO.Path]::GetFullPath($path)
        if (-not $fullPath.StartsWith($workspaceRoot + "\", [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "refusing to write a package outside the workspace."
        }
    }

    New-Item -ItemType Directory -Force -Path $packageRoot | Out-Null
    if (Test-Path -LiteralPath $packageDir) {
        Remove-Item -LiteralPath $packageDir -Recurse -Force
    }
    if (Test-Path -LiteralPath $archive) {
        Remove-Item -LiteralPath $archive -Force
    }

    $mdxSource = (Get-ChildItem -LiteralPath $workspace -File -Filter "*.mdx" | Select-Object -First 1).FullName
    $txtSource = (Get-ChildItem -LiteralPath (Join-Path $workspace "data\tmp_dict") -File -Filter "*.txt" -ErrorAction SilentlyContinue | Select-Object -First 1).FullName
    $dictionarySource = if ($txtSource) { $txtSource } else { $mdxSource }
    if (-not $dictionarySource) {
        throw "no MDX or equivalent TXT dictionary source found"
    }
    $dictionaryBundle = Join-Path $workspace "data\dict-sources\daijirin.kdict"
    $bundleDirectory = Split-Path -Parent $dictionaryBundle
    New-Item -ItemType Directory -Force -Path $bundleDirectory | Out-Null
    $bundleOutdated = -not (Test-Path -LiteralPath $dictionaryBundle)
    if (-not $bundleOutdated) {
        $bundleOutdated = (Get-Item -LiteralPath $dictionaryBundle).LastWriteTimeUtc -lt (Get-Item -LiteralPath $dictionarySource).LastWriteTimeUtc
    }
    if ($bundleOutdated) {
        & python "scripts\build_dictionary_bundle.py" $dictionarySource $dictionaryBundle
        if ($LASTEXITCODE -ne 0) {
            throw "dictionary bundle build failed with exit code $LASTEXITCODE"
        }
    }

    New-Item -ItemType Directory -Force -Path (Join-Path $packageDir "ipadic"), (Join-Path $packageDir "dict-sources") | Out-Null
    Copy-Item -LiteralPath "target\release\tauri-app.exe" -Destination (Join-Path $packageDir "Kotoclip.exe") -Force
    Copy-Item -LiteralPath "ipadic\system.dic" -Destination (Join-Path $packageDir "ipadic\system.dic") -Force
    Copy-Item -LiteralPath $dictionaryBundle -Destination (Join-Path $packageDir "dict-sources\daijirin.kdict") -Force
    Compress-Archive -Path (Join-Path $packageDir "*") -DestinationPath $archive -Force
    Write-Output "package: $archive"
}

try {
    Clear-TargetIfOverLimit

    $env:CARGO_TARGET_DIR = Join-Path $workspace "target"

    if ($Channel -eq "dev") {
        $env:CARGO_INCREMENTAL = "1"
        Remove-Item Env:VITE_BUILD_CHANNEL -ErrorAction SilentlyContinue
        & npm.cmd run tauri -- dev
    } else {
        $env:CARGO_INCREMENTAL = "0"
        $env:VITE_BUILD_CHANNEL = "insider"
        & npm.cmd run tauri -- build --no-bundle
    }

    $exitCode = $LASTEXITCODE
    if ($Channel -eq "insider" -and $exitCode -eq 0) {
        Create-InsiderPackage
    }
} finally {
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:CARGO_INCREMENTAL -ErrorAction SilentlyContinue
    Remove-Item Env:VITE_BUILD_CHANNEL -ErrorAction SilentlyContinue
    Pop-Location
}

exit $exitCode
