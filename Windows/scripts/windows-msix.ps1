# 先在证书存储中准备发布证书；脚本不会创建或信任任何证书。
param(
    [Parameter(Mandatory = $true)][string]$CertificateThumbprint,
    [string]$Version = '0.1.0.0',
    [string]$OutputDirectory = 'artifacts',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$certificate = Get-Item "Cert:\CurrentUser\My\$CertificateThumbprint"
if (-not $certificate.HasPrivateKey) { throw '签名证书必须带有私钥。' }
if ($certificate.NotAfter -lt (Get-Date)) { throw '签名证书已经过期。' }
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Directory | Sort-Object Name -Descending | Where-Object { Test-Path (Join-Path $_.FullName 'x64\makeappx.exe') } | Select-Object -First 1
if (-not $sdk) { throw '请安装 Windows 11 SDK，包含 MakeAppx 和 SignTool。' }
$makeappx = Join-Path $sdk.FullName 'x64\makeappx.exe'
$signtool = Join-Path $sdk.FullName 'x64\signtool.exe'
$stage = Join-Path ([IO.Path]::GetTempPath()) ('lightledger-msix-' + [guid]::NewGuid())
Push-Location $root
try {
    if (-not $SkipBuild) {
        npm ci
        if ($LASTEXITCODE -ne 0) { throw '前端依赖安装失败。' }
        npm run tauri:windows -- build --target x86_64-pc-windows-msvc --no-bundle
        if ($LASTEXITCODE -ne 0) { throw 'Windows 应用构建失败。' }
    }
    $release = Join-Path $root 'target\x86_64-pc-windows-msvc\release'
    $exe = Join-Path $release 'lightledger-app.exe'
    if (-not (Test-Path $exe)) { throw "未找到 Windows x64 程序：$exe" }
    New-Item $stage -ItemType Directory | Out-Null
    Copy-Item $exe $stage
    Get-ChildItem $release -Filter '*.dll' | Copy-Item -Destination $stage
    Copy-Item (Join-Path $root 'Windows\packaging\Assets') $stage -Recurse
    [xml]$manifest = Get-Content (Join-Path $root 'Windows\packaging\AppxManifest.xml') -Raw -Encoding UTF8
    $manifest.Package.Identity.Publisher = $certificate.Subject
    $manifest.Package.Identity.Version = $Version
    $manifest.Save((Join-Path $stage 'AppxManifest.xml'))
    New-Item $OutputDirectory -ItemType Directory -Force | Out-Null
    $output = Join-Path (Resolve-Path $OutputDirectory) "LightLedger-$Version-x64.msix"
    & $makeappx pack /d $stage /p $output /o
    if ($LASTEXITCODE -ne 0) { throw 'MSIX 清单验证或打包失败。' }
    & $signtool sign /sha1 $CertificateThumbprint /fd SHA256 $output
    if ($LASTEXITCODE -ne 0) { throw 'MSIX 签名失败。' }
    & $signtool verify /pa /v $output
    if ($LASTEXITCODE -ne 0) { throw '签名验证失败，请使用设备信任链中的发布证书。' }
    Write-Output "已生成：$output"
    Write-Output '目标设备需安装 Microsoft Edge WebView2 Runtime；OCR/通知仍需 MSIX 实机验收。'
} finally {
    Pop-Location
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
}
