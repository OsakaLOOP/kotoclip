<#
.SYNOPSIS
对完整章节执行可复现的随机加载与规则增量差分验证。

.DESCRIPTION
每个种子都由当前版本完整管线实时生成基准，不读取固定 golden 输出。
画像库会由 CLI 复制到临时目录，真实规则和用户状态不会被修改。
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SourcePath,
    [string]$Chapter = '## 第一話　冷やし神',
    [string]$ProfilePath = 'data/research-profile.sqlite',
    [UInt64[]]$Seeds = @(2026071301, 2026071302, 2026071303),
    [int]$LoadCases = 3,
    [int]$RuleCases = 2
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$source = (Resolve-Path -LiteralPath $SourcePath).Path
$profile = if ([IO.Path]::IsPathRooted($ProfilePath)) {
    $ProfilePath
} else {
    Join-Path $repositoryRoot $ProfilePath
}

Push-Location $repositoryRoot
try {
    cargo build -q -p kotoclip-core --bin kotoclip-cli
    if ($LASTEXITCODE -ne 0) {
        throw "增量一致性验证器编译失败：$LASTEXITCODE"
    }
    foreach ($seed in $Seeds) {
        & .\target\debug\kotoclip-cli.exe incremental-consistency `
            --source $source `
            --chapter $Chapter `
            --profile $profile `
            --seed $seed `
            --load-cases $LoadCases `
            --rule-cases $RuleCases
        if ($LASTEXITCODE -ne 0) {
            throw "增量一致性验证失败，seed=$seed，退出码：$LASTEXITCODE"
        }
    }
}
finally {
    Pop-Location
}
