<#
.SYNOPSIS
  Scaffold a new Nulqor extension under extensions/<id>/ with colocated layout.
.PARAMETER Id
  Extension id (kebab-case). Must match folder name and extension.toml id field.
.PARAMETER Kind
  Extension kind: Service, Panel, Host, or Provider.
.PARAMETER Purpose
  One-line description for README and extensions/index.md.
.PARAMETER Requires
  Comma-separated extension ids for manifest requires (optional).
.PARAMETER Root
  Repo root. Defaults to current directory.
.PARAMETER Quiet
  Suppress success output.
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Id,

    [Parameter(Mandatory = $true)]
    [ValidateSet("Service", "Panel", "Host", "Provider")]
    [string]$Kind,

    [Parameter(Mandatory = $true)]
    [string]$Purpose,

    [string]$Requires = "",

    [string]$Root = ".",

    [switch]$Quiet
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path $Root).Path

function IdToModName {
    param([string]$ExtId)
    return ($ExtId -replace '-', '_')
}

function Get-ExtensionStructName {
    param([string]$ExtId)
    $parts = $ExtId -split '-'
    $pascal = ($parts | ForEach-Object {
        if ([string]::IsNullOrWhiteSpace($_)) { return $_ }
        $_.Substring(0, 1).ToUpper() + $_.Substring(1)
    }) -join ''
    return "${pascal}Extension"
}

if ($Id -notmatch '^[a-z][a-z0-9]*(-[a-z0-9]+)*$') {
    Write-Host "FAIL: Id must be kebab-case (e.g. my-extension)"
    exit 1
}

$extDir = Join-Path $Root "extensions/$Id"
if (Test-Path $extDir) {
    Write-Host "FAIL: extensions/$Id already exists"
    exit 1
}

$modName = IdToModName -ExtId $Id
$structName = Get-ExtensionStructName -ExtId $Id

# Create extension tree
New-Item -ItemType Directory -Path (Join-Path $extDir "src") -Force | Out-Null

$requiresLine = ""
if (-not [string]::IsNullOrWhiteSpace($Requires)) {
    $reqParts = $Requires -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    $reqJoined = ($reqParts | ForEach-Object { "`"$_`"" }) -join ", "
    $requiresLine = "requires       = [$reqJoined]`r`n"
}

$manifest = @(
    "[extension]"
    "id             = `"$Id`""
    "version        = `"0.1.0`""
    "kind           = `"$Kind`""
    "api-version    = `"v1`""
    "schema-version = `"1.0.0`""
    "min-core       = `"0.1.0`""
)
if ($requiresLine) {
    $manifest += $requiresLine.TrimEnd()
}
Set-Content -Path (Join-Path $extDir "extension.toml") -Value ($manifest -join "`r`n") -Encoding utf8

$readmeLines = @(
    "# $Id"
    ""
    $Purpose
    ""
    "| Path | Purpose |"
    "|---|---|"
    "| ``extension.toml`` | Manifest |"
    "| ``src/lib.rs`` | Rust implementation |"
)
if ($Kind -eq "Panel") {
    $readmeLines += "| ``ui/`` | TypeScript panel |"
}
Set-Content -Path (Join-Path $extDir "README.md") -Value ($readmeLines -join "`r`n") -Encoding utf8

$libRs = @(
    "//! $Id extension - scaffold created by create-extension skill."
    ""
    "use crate::context::{CoreContext, Extension};"
    "use crate::error::CoreError;"
    "use crate::types::ExtensionManifest;"
    ""
    "pub struct $structName {"
    "    manifest: ExtensionManifest,"
    "}"
    ""
    "impl $structName {"
    "    pub fn new(manifest: ExtensionManifest) -> Self {"
    "        Self { manifest }"
    "    }"
    "}"
    ""
    "impl Extension for $structName {"
    "    fn manifest(&self) -> &ExtensionManifest {"
    "        &self.manifest"
    "    }"
    ""
    "    fn activate(&self, _ctx: &CoreContext) -> Result<(), CoreError> {"
    ('        eprintln!("[' + $Id + '] activated (scaffold - implement commands/events)");')
    "        Ok(())"
    "    }"
    "}"
)
Set-Content -Path (Join-Path $extDir "src/lib.rs") -Value ($libRs -join "`r`n") -Encoding utf8

if ($Kind -eq "Panel") {
    $uiDir = Join-Path $extDir "ui"
    New-Item -ItemType Directory -Path $uiDir -Force | Out-Null
    Set-Content -Path (Join-Path $uiDir "main.ts") -Value "// $Id panel UI - implement here" -Encoding utf8
    Set-Content -Path (Join-Path $uiDir "style.css") -Value "/* $Id panel styles */" -Encoding utf8
}

# Update mod.rs bridge
$modPath = Join-Path $Root "src-tauri/src/extensions/mod.rs"
if (-not (Test-Path $modPath)) {
    Write-Host "FAIL: src-tauri/src/extensions/mod.rs not found"
    exit 1
}

$modBlock = @(
    ""
    "#[path = `"../../../extensions/$Id/src/lib.rs`"]"
    "pub mod $modName;"
    ""
)
Add-Content -Path $modPath -Value ($modBlock -join "`r`n") -Encoding utf8

# Update extensions/index.md
$indexPath = Join-Path $Root "extensions/index.md"
if (-not (Test-Path $indexPath)) {
    Write-Host "FAIL: extensions/index.md not found"
    exit 1
}

$indexLines = Get-Content -Path $indexPath
$insertAt = -1
for ($i = 0; $i -lt $indexLines.Count; $i++) {
    if ($indexLines[$i] -match '^\|---\|---\|---\|---\|$') {
        $insertAt = $i + 1
        break
    }
}
if ($insertAt -lt 0) {
    Write-Host "FAIL: extensions/index.md missing registry table separator"
    exit 1
}

$newRow = '| `' + $Id + '` | ' + $Kind + ' | Pending | ' + $Purpose + ' |'
$before = $indexLines[0..($insertAt - 1)]
$after = if ($insertAt -lt $indexLines.Count) { $indexLines[$insertAt..($indexLines.Count - 1)] } else { @() }
($before + $newRow + $after) | Set-Content -Path $indexPath -Encoding utf8

# Instructions for lib.rs (manual merge point)
$registerLine = 'loader.register("' + $Id + '", |m| Arc::new(' + $modName + '::' + $structName + '::new(m)));'

Write-Host ""
Write-Host "Scaffold created: extensions/$Id/"
Write-Host ""
Write-Host "REQUIRED: add to src-tauri/src/lib.rs load_extensions():"
Write-Host "  $registerLine"
Write-Host ""
Write-Host "Then run:"
Write-Host ('  skills/audit-project/scripts/audit.ps1 -Root "' + $Root + '"')
Write-Host "  cargo test --workspace"
Write-Host ""

if (-not $Quiet) {
    Write-Host "OK: extension scaffold created for '$Id'"
}
exit 0
