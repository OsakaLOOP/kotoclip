<#
.SYNOPSIS
交互运行阅读器端到端后端基准，默认不写入真实用户画像。

.DESCRIPTION
测量源文件读取、章节提取、Engine 冷启动、核心分析内部阶段以及
Tauri IPC JSON 负载序列化。DOM 布局和绘制必须在桌面前端性能面板中测量，
不在本 CLI 进程的统计范围内。
#>
[CmdletBinding()]
param(
    [string]$SourcePath,
    [string]$Chapter = '## 第一話　冷やし神',
    [string]$ProfilePath,
    [string]$ReportPath,
    [switch]$RecordExposure
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

if ([string]::IsNullOrWhiteSpace($SourcePath)) {
    $SourcePath = Read-Host '源文本路径'
}
if (-not (Test-Path -LiteralPath $SourcePath -PathType Leaf)) {
    throw "找不到源文本：$SourcePath"
}

if ([string]::IsNullOrWhiteSpace($Chapter)) {
    $Chapter = Read-Host '章节标题（留空表示整个文件）'
}

$temporaryProfile = $false
if ([string]::IsNullOrWhiteSpace($ProfilePath)) {
    $ProfilePath = Join-Path ([IO.Path]::GetTempPath()) ("kotoclip-reader-benchmark-{0}.sqlite" -f [guid]::NewGuid())
    $seedProfile = Join-Path $repositoryRoot 'data\research-profile.sqlite'
    if (Test-Path -LiteralPath $seedProfile -PathType Leaf) {
        Copy-Item -LiteralPath $seedProfile -Destination $ProfilePath
    }
    $temporaryProfile = $true
}

$arguments = @(
    'run', '-q', '-p', 'kotoclip-core', '--bin', 'kotoclip-cli', '--',
    'reader-benchmark',
    '--source', $SourcePath,
    '--profile', $ProfilePath
)
if (-not [string]::IsNullOrWhiteSpace($Chapter)) {
    $arguments += @('--chapter', $Chapter)
}
if (-not $RecordExposure) {
    $arguments += '--no-record-exposure'
}
if (-not [string]::IsNullOrWhiteSpace($ReportPath)) {
    $arguments += @('--json', $ReportPath)
}

try {
    Push-Location $repositoryRoot
    & cargo @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "reader-benchmark 失败，退出码：$LASTEXITCODE"
    }
}
finally {
    Pop-Location -ErrorAction SilentlyContinue
    if ($temporaryProfile -and (Test-Path -LiteralPath $ProfilePath)) {
        Remove-Item -LiteralPath $ProfilePath -Force
    }
}
