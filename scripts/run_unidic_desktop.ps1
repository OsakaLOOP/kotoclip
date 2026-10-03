[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $workspace

function Stop-WorkspaceProcess {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Predicate
    )

    $processes = Get-CimInstance Win32_Process | Where-Object $Predicate
    foreach ($process in $processes) {
        Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

try {
    # 结束上一轮验收遗留的 bridge、Vite 和桌面进程；Codex 自身进程不匹配这些条件。
    Stop-WorkspaceProcess {
        param($process)
        ($process.Name -eq "kotoclip-nlp.exe" -and $process.ExecutablePath -like "$workspace\target\*") -or
        ($process.Name -eq "tauri-app.exe" -and $process.ExecutablePath -like "$workspace\target\*") -or
        ($process.Name -eq "node.exe" -and $process.CommandLine -match [regex]::Escape("$workspace\node_modules\.bin") -and $process.CommandLine -match "vite\.js" -and $process.CommandLine -match "--port 1420") -or
        ($process.Name -eq "node.exe" -and $process.CommandLine -match "npm-cli\.js.*run acceptance")
    }

    $env:KOTOCLIP_UNIDIC_CWJ = Join-Path $workspace "experiments\unidic-source\unidic-cwj-202512.vibrato.dic"
    $env:KOTOCLIP_UNIDIC_CSJ = Join-Path $workspace "experiments\unidic-source\unidic-csj-202512.vibrato.dic"

    & (Join-Path $workspace "scripts\run_channel.ps1") -Channel dev
    exit $LASTEXITCODE
}
finally {
    Remove-Item Env:KOTOCLIP_UNIDIC_CWJ -ErrorAction SilentlyContinue
    Remove-Item Env:KOTOCLIP_UNIDIC_CSJ -ErrorAction SilentlyContinue
    Pop-Location
}
