<#
.SYNOPSIS
  Verify Nulqor repo layout, extension colocation, and index/mod.rs sync.
.PARAMETER Root
  Repo root path. Defaults to current directory.
.PARAMETER Quiet
  Suppress output on pass.
.PARAMETER SkipLint
  Skip invoking nulqor-lint (faster local runs).
#>
param(
    [string]$Root = ".",
    [switch]$Quiet,
    [switch]$SkipLint,
    [switch]$Strict
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path $Root).Path
$failures = [System.Collections.Generic.List[string]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()

function Add-Fail {
    param([string]$Message)
    $failures.Add($Message) | Out-Null
}

function Add-Warn {
    param([string]$Message)
    $warnings.Add($Message) | Out-Null
}

function IdToModName {
    param([string]$Id)
    return ($Id -replace '-', '_')
}

function Get-ExtensionStructName {
    param([string]$Id)
    $parts = $Id -split '-'
    $pascal = ($parts | ForEach-Object {
        if ([string]::IsNullOrWhiteSpace($_)) { return $_ }
        $_.Substring(0, 1).ToUpper() + $_.Substring(1)
    }) -join ''
    return "${pascal}Extension"
}

function Get-ManifestKind {
    param([string]$ManifestPath)
    if (-not (Test-Path $ManifestPath)) { return $null }
    $content = Get-Content -Raw -Path $ManifestPath
    if ($content -match 'kind\s*=\s*"([^"]+)"') {
        return $Matches[1]
    }
    return $null
}

function Get-ManifestId {
    param([string]$ManifestPath)
    if (-not (Test-Path $ManifestPath)) { return $null }
    $content = Get-Content -Raw -Path $ManifestPath
    if ($content -match 'id\s*=\s*"([^"]+)"') {
        return $Matches[1]
    }
    return $null
}

function Get-IndexExtensionIds {
    param([string]$IndexPath)
    if (-not (Test-Path $IndexPath)) {
        Add-Fail "extensions/index.md: file not found"
        return @()
    }
    $lines = Get-Content -Path $IndexPath
    $inTable = $false
    $ids = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $lines) {
        if ($line -match '^## Registered Extensions') {
            $inTable = $true
            continue
        }
        if ($inTable -and $line -match '^## ') {
            break
        }
        if ($inTable -and $line -match '^\|\s*`([^`]+)`\s*\|') {
            $ids.Add($Matches[1]) | Out-Null
        }
    }
    return @($ids)
}

function Get-ModRsEntries {
    param([string]$ModPath)
    if (-not (Test-Path $ModPath)) {
        Add-Fail "src-tauri/src/extensions/mod.rs: file not found"
        return @()
    }
    $content = Get-Content -Raw -Path $ModPath
    $entries = @()
    $pattern = '#\[path\s*=\s*"([^"]+/extensions/([^/]+)/src/lib\.rs)"\]\s*\r?\n\s*pub\s+mod\s+([a-zA-Z0-9_]+)\s*;'
    foreach ($match in [regex]::Matches($content, $pattern)) {
        $entries += [pscustomobject]@{
            Path     = $match.Groups[1].Value -replace '\\', '/'
            ExtId    = $match.Groups[2].Value
            ModName  = $match.Groups[3].Value
        }
    }
    return $entries
}

# ── 1. Required top-level directories and indexes ───────────────────────────

$requiredDirs = @("docs", "extensions", "skills", "rules", "src-tauri", "src", "tools", "archive")
foreach ($dir in $requiredDirs) {
    $path = Join-Path $Root $dir
    if (-not (Test-Path $path -PathType Container)) {
        Add-Fail "${Root}: expected directory '$dir' not found"
    }
}

$requiredIndexes = @{
    "docs/index.md"        = "docs"
    "extensions/index.md"  = "extensions"
    "skills/index.md"      = "skills"
    "rules/index.md"       = "rules"
    "tools/index.md"       = "tools"
    "archive/index.md"     = "archive"
}
foreach ($entry in $requiredIndexes.GetEnumerator()) {
    $indexPath = Join-Path $Root $entry.Key
    if (-not (Test-Path $indexPath -PathType Leaf)) {
        Add-Fail "$($entry.Key): index file not found (required for $($entry.Value)/)"
    }
}

# ── 2. Forbidden legacy paths ───────────────────────────────────────────────

$forbiddenGlobs = @(
    @{ Glob = "src-tauri/src/ext_*.rs"; Reason = "use extensions/<id>/src/lib.rs instead" }
    @{ Glob = "src-tauri/src/extensions/ext_*.rs"; Reason = "use extensions/<id>/src/lib.rs instead" }
    @{ Glob = "src/*.ts"; Reason = "panel UI belongs in extensions/<id>/ui/" }
    @{ Glob = "src/*.tsx"; Reason = "panel UI belongs in extensions/<id>/ui/" }
    @{ Glob = "src/*.css"; Reason = "panel styles belong in extensions/<id>/ui/" }
)

foreach ($rule in $forbiddenGlobs) {
    $pattern = Join-Path $Root ($rule.Glob -replace '/', '\')
    $parent = Split-Path $pattern -Parent
    $leaf = Split-Path $pattern -Leaf
    if (Test-Path $parent) {
        Get-ChildItem -Path $parent -Filter $leaf -File -ErrorAction SilentlyContinue | ForEach-Object {
            Add-Fail "$($_.FullName.Substring($Root.Length + 1)): forbidden - $($rule.Reason)"
        }
    }
}

# Allow src/README.md only under src/ (no other loose files except README)
$srcDir = Join-Path $Root "src"
if (Test-Path $srcDir) {
    Get-ChildItem -Path $srcDir -File | ForEach-Object {
        if ($_.Name -ne "README.md") {
            Add-Fail "src/$($_.Name): only src/README.md is allowed at repo src/ root"
        }
    }
}

# ── 3. Per-extension scaffold ───────────────────────────────────────────────

# Shared-code modules compiled into the core but NOT loadable extensions:
# no manifest, no commands, no loader.register, no index row. They are still
# bridged via #[path] in mod.rs so sibling extensions can `use` them.
# provider-common holds the shared HTTP/OpenAI helpers for the provider-* backends.
$SHARED_MODULES = @("provider-common")

$extensionsDir = Join-Path $Root "extensions"
$diskExtIds = [System.Collections.Generic.List[string]]::new()

if (Test-Path $extensionsDir) {
    Get-ChildItem -Path $extensionsDir -Directory | ForEach-Object {
        if ($_.Name -eq "index.md") { return }
        if ($SHARED_MODULES -contains $_.Name) { return }
        $extId = $_.Name
        $manifestPath = Join-Path $_.FullName "extension.toml"
        if (-not (Test-Path $manifestPath)) {
            Add-Fail "extensions/${extId}: extension.toml not found"
            return
        }

        $manifestId = Get-ManifestId -ManifestPath $manifestPath
        if ($manifestId -and $manifestId -ne $extId) {
            Add-Fail "extensions/${extId}/extension.toml: id '$manifestId' must match folder name '$extId'"
        }

        $diskExtIds.Add($extId) | Out-Null

        $libRs = Join-Path $_.FullName "src/lib.rs"
        if (-not (Test-Path $libRs)) {
            Add-Fail "extensions/${extId}: src/lib.rs not found (required)"
        }

        $readme = Join-Path $_.FullName "README.md"
        if (-not (Test-Path $readme)) {
            Add-Fail "extensions/${extId}: README.md not found (required)"
        }

        $kind = Get-ManifestKind -ManifestPath $manifestPath
        if ($kind -eq "Panel") {
            $uiDir = Join-Path $_.FullName "ui"
            if (-not (Test-Path $uiDir -PathType Container)) {
                Add-Fail "extensions/${extId}: ui/ required for kind Panel"
            }
            else {
                $uiFiles = Get-ChildItem -Path $uiDir -Recurse -File -ErrorAction SilentlyContinue
                if (-not $uiFiles -or $uiFiles.Count -eq 0) {
                    Add-Fail "extensions/${extId}/ui: must contain at least one file"
                }
            }
        }
    }
}

# ── 4. extensions/index.md sync ───────────────────────────────────────────

$indexPath = Join-Path $Root "extensions/index.md"
$indexIds = @(Get-IndexExtensionIds -IndexPath $indexPath)

foreach ($id in $diskExtIds) {
    if ($indexIds -notcontains $id) {
        Add-Fail "extensions/index.md: missing registry row for '$id'"
    }
}

foreach ($id in $indexIds) {
    if ($diskExtIds -notcontains $id) {
        Add-Fail "extensions/index.md: lists '$id' but extensions/$id/ does not exist"
    }
}

# ── 5. mod.rs bridge sync ───────────────────────────────────────────────────

$modPath = Join-Path $Root "src-tauri/src/extensions/mod.rs"
$modEntries = @(Get-ModRsEntries -ModPath $modPath)
$modExtIds = @($modEntries | ForEach-Object { $_.ExtId })

foreach ($id in $diskExtIds) {
    if ($modExtIds -notcontains $id) {
        Add-Fail "src-tauri/src/extensions/mod.rs: missing #[path] bridge for extensions/$id/src/lib.rs"
    }
}

foreach ($entry in $modEntries) {
    $expectedMod = IdToModName -Id $entry.ExtId
    if ($entry.ModName -ne $expectedMod) {
        Add-Fail "src-tauri/src/extensions/mod.rs: mod '$($entry.ModName)' should be '$expectedMod' for extension '$($entry.ExtId)'"
    }

    $expectedSuffix = "extensions/$($entry.ExtId)/src/lib.rs"
    $normalizedPath = $entry.Path -replace '\\', '/'
    if (-not $normalizedPath.EndsWith($expectedSuffix)) {
        Add-Fail "src-tauri/src/extensions/mod.rs: path for '$($entry.ExtId)' must end with $expectedSuffix"
    }

    if ($diskExtIds -notcontains $entry.ExtId -and $SHARED_MODULES -notcontains $entry.ExtId) {
        Add-Fail "src-tauri/src/extensions/mod.rs: bridge for '$($entry.ExtId)' but extensions/$($entry.ExtId)/ missing"
    }
}

# ── 6. lib.rs loader.register sync ────────────────────────────────────────

$libPath = Join-Path $Root "src-tauri/src/lib.rs"
if (Test-Path $libPath) {
    $libContent = Get-Content -Raw -Path $libPath
    foreach ($id in $diskExtIds) {
        $escaped = [regex]::Escape($id)
        if ($libContent -notmatch "loader\.register\s*\(\s*`"$escaped`"") {
            Add-Fail "src-tauri/src/lib.rs: missing loader.register(`"$id`", ...)"
        }
    }
}

# ── 7. nulqor-lint ────────────────────────────────────────────────────────

if (-not $SkipLint) {
    $lintManifest = Join-Path $Root "tools/nulqor-lint/Cargo.toml"
    if (Test-Path $lintManifest) {
        $extPath = Join-Path $Root "extensions"
        $prevEap = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            $lintOutput = & cargo run --manifest-path $lintManifest -- $extPath 2>&1
            $lintExit = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $prevEap
        }
        foreach ($line in @($lintOutput)) {
            $text = "$line"
            if ($text -match '^FAIL:') {
                Add-Fail $text
            }
        }
        if ($lintExit -ne 0 -and (@($lintOutput | Where-Object { "$_" -match '^FAIL:' }).Count -eq 0)) {
            Add-Fail "nulqor-lint: exited with code $lintExit"
        }
    }
    else {
        Add-Fail "tools/nulqor-lint/Cargo.toml: not found (cannot run linter)"
    }
}

# ── 8. Doc consistency (drift guard) ─────────────────────────────────────────

$tasksPath = Join-Path $Root "TASKS.md"
$phasesPath = Join-Path $Root "docs/PHASES.md"
if ((Test-Path $tasksPath) -and (Test-Path $phasesPath)) {
    $phasesRaw = Get-Content -Raw -Path $phasesPath
    $tasksRaw = Get-Content -Raw -Path $tasksPath
    $phasesCurrent = $null
    if ($phasesRaw -match 'Current:\s*Phase\s*(\d+)') { $phasesCurrent = [int]$Matches[1] }
    $maxDonePhase = 0
    foreach ($m in [regex]::Matches($tasksRaw, '(?m)^\|\s*(\d+)\.\d+\s*\|.*\b(Done|Partial)\b')) {
        $p = [int]$m.Groups[1].Value
        if ($p -gt $maxDonePhase) { $maxDonePhase = $p }
    }
    if (($null -ne $phasesCurrent) -and ($maxDonePhase -gt $phasesCurrent)) {
        Add-Warn "docs/PHASES.md says 'Current: Phase $phasesCurrent' but TASKS.md shows Done/Partial work through Phase $maxDonePhase (doc drift)"
    }
}

$goalPath = Join-Path $Root "docs/GOAL.md"
$decPath = Join-Path $Root "docs/decisions/001-frozen-core.md"
if ((Test-Path $goalPath) -and (Test-Path $decPath)) {
    $goalRaw = Get-Content -Raw -Path $goalPath
    $decRaw = Get-Content -Raw -Path $decPath
    $goalWord = $null; $decWord = $null
    if ($goalRaw -match '(?i)\b([a-z]+)-responsibility core') { $goalWord = $Matches[1].ToLower() }
    if ($decRaw -match '(?i)frozen at (\w+) responsibilit') { $decWord = $Matches[1].ToLower() }
    if ($goalWord -and $decWord -and ($goalWord -ne $decWord)) {
        Add-Warn "core-responsibility count drift: GOAL.md says '$goalWord-responsibility' but decisions/001 says '$decWord responsibilities'"
    }
}

# ── 9. Port uniqueness (collision guard) ─────────────────────────────────────

if (Test-Path $extensionsDir) {
    $portMap = @{}
    Get-ChildItem -Path $extensionsDir -Directory | ForEach-Object {
        $extId = $_.Name
        # provider-router catalogs other backends; mcp-bridge/mcp-server are API clients (no listener).
        # Their port references are legitimate, not collisions.
        if ($extId -eq "provider-router" -or $extId -eq "mcp-bridge") { return }
        $ports = [System.Collections.Generic.HashSet[string]]::new()
        Get-ChildItem -Path $_.FullName -Recurse -File -Include *.rs, *.md, *.toml -ErrorAction SilentlyContinue | ForEach-Object {
            $c = Get-Content -Raw -Path $_.FullName
            foreach ($pm in [regex]::Matches($c, 'localhost:(\d{2,5})')) { [void]$ports.Add($pm.Groups[1].Value) }
            foreach ($pm in [regex]::Matches($c, '(?i)DEFAULT_PORT[^=\n]*=\s*(\d{2,5})')) { [void]$ports.Add($pm.Groups[1].Value) }
        }
        foreach ($p in $ports) {
            if (-not $portMap.ContainsKey($p)) { $portMap[$p] = [System.Collections.Generic.List[string]]::new() }
            $portMap[$p].Add($extId) | Out-Null
        }
    }
    foreach ($kv in $portMap.GetEnumerator()) {
        if ($kv.Value.Count -gt 1) {
            Add-Warn "port $($kv.Key) shared by extensions: $($kv.Value -join ', ') - verify no listening collision (see rules/engineering-guardrails.md)"
        }
    }
}

# ── 10. Polling inventory (prefer event push) ────────────────────────────────

if (Test-Path $extensionsDir) {
    $pollFiles = [System.Collections.Generic.List[string]]::new()
    Get-ChildItem -Path $extensionsDir -Recurse -File -Filter *.ts -ErrorAction SilentlyContinue | ForEach-Object {
        $c = Get-Content -Raw -Path $_.FullName
        $count = ([regex]::Matches($c, 'setInterval\s*\(')).Count
        if ($count -gt 0) {
            $rel = $_.FullName.Substring($Root.Length + 1) -replace '\\', '/'
            $pollFiles.Add("$rel ($count setInterval)") | Out-Null
        }
    }
    if ($pollFiles.Count -gt 0) {
        Add-Warn "polling sites (prefer event push - rules/engineering-guardrails.md): $($pollFiles -join '; ')"
    }
}

# ── Result ──────────────────────────────────────────────────────────────────

if ($Strict) {
    foreach ($w in $warnings) { Add-Fail $w }
    $warnings.Clear()
}

if ($warnings.Count -gt 0) {
    foreach ($w in $warnings) {
        Write-Host "WARN: $w"
    }
}

if ($failures.Count -gt 0) {
    foreach ($f in $failures) {
        Write-Host "FAIL: $f"
    }
    exit 1
}

if (-not $Quiet) {
    Write-Host "OK: project structure, extension colocation, and registry sync verified"
}
exit 0
