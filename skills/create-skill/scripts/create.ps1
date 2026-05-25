# create.ps1 -- Skill scaffold generator (Nulqor host format)
# Platform: Windows PowerShell 5.1+
#
# Usage:
#   skills/create-skill/scripts/create.ps1 -SkillName <name> -Description "<desc>" -Topics "<topics>"
#   skills/create-skill/scripts/create.ps1 -SkillName win-foo -Description "..." -Topics "..." -Platform windows
#
# Creates SKILL.md with name/description frontmatter and ## Metadata body block.
# Updates skills/index.md (Skill | Purpose) when present.

param(
    [Parameter(Mandatory)]
    [string]$SkillName,

    [Parameter(Mandatory)]
    [string]$Description,

    [Parameter(Mandatory)]
    [string]$Topics,

    [ValidateSet("all", "windows", "macos", "linux")]
    [string]$Platform = "all",

    [ValidateSet("none", "optional", "required")]
    [string]$ScriptPolicy = "none",

    [ValidateSet("generic", "os-scoped", "tool-scoped", "domain-scoped", "project-scoped")]
    [string]$Scope = "generic",

    [string]$Root = ".",

    [switch]$HasScript,

    [switch]$SkipIndex,

    [switch]$Quiet
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path $Root).Path
$SkillsRoot = Join-Path $Root "skills"
$SkillDir   = Join-Path $SkillsRoot $SkillName
$SkillFile  = Join-Path $SkillDir "SKILL.md"
$ScriptsDir = Join-Path $SkillDir "scripts"
$IndexFile  = Join-Path $SkillsRoot "index.md"
$AuditScript = Join-Path $SkillsRoot "audit-skill/scripts/audit.ps1"

if ($SkillName -notmatch '^[a-z][a-z0-9-]*[a-z0-9]$') {
    Write-Error "Skill name '$SkillName' is invalid. Use lowercase letters, numbers, and hyphens."
    exit 1
}
if ($SkillName -match '--') {
    Write-Error "Skill name '$SkillName' contains consecutive hyphens."
    exit 1
}
if ($SkillName.Length -gt 64) {
    Write-Error "Skill name '$SkillName' exceeds 64 characters."
    exit 1
}

if (Test-Path $SkillDir) {
    Write-Error "Skill directory already exists: $SkillDir`nNot overwriting."
    exit 1
}

New-Item -ItemType Directory -Path $SkillDir    | Out-Null
New-Item -ItemType Directory -Path $ScriptsDir | Out-Null

$SkillContent = @"
---
name: $SkillName
description: $Description
---

## Metadata

```text
version:       1.0.0
topics:        $Topics
platform:      $Platform
script_policy: $ScriptPolicy
scope:         $Scope
```

<One or two sentences: what problem this skill solves.>

---

## When to use

- <specific trigger condition>

---

## Contract

```text
when:         <trigger expansion>
inputs:       <param> -- <description, or "none">
outputs:      <what is produced>
side-effects: none
validation:   <observable checks before reporting success>
```

---

## Steps

1. <First step>

---

## Verification

- [ ] <Observable condition>
"@

Set-Content -Path $SkillFile -Value $SkillContent -Encoding UTF8

if ($HasScript) {
    $ScriptFile = Join-Path $ScriptsDir "$SkillName.ps1"
    @"
# $SkillName.ps1
# Platform: Windows (PowerShell 5.1+)
# TODO: Implement script logic.

Set-StrictMode -Version Latest
`$ErrorActionPreference = "Stop"
"@ | Set-Content -Path $ScriptFile -Encoding UTF8
    if (-not $Quiet) {
        Write-Host "  Created: $ScriptFile"
    }
}

if (-not $SkipIndex) {
    if (-not (Test-Path $IndexFile)) {
        Write-Warning "skills/index.md not found at $IndexFile -- skip index row"
    } else {
        $indexLines = Get-Content -Path $IndexFile
        $insertAt = -1
        for ($i = 0; $i -lt $indexLines.Count; $i++) {
            if ($indexLines[$i] -match '^\|---\|---\|$') {
                $insertAt = $i + 1
                break
            }
        }
        if ($insertAt -lt 0) {
            Write-Host "FAIL: skills/index.md missing Available Skills table separator (|---|---|)"
            exit 1
        }
        $newRow = '| `' + $SkillName + '` | ' + $Description + ' |'
        $before = $indexLines[0..($insertAt - 1)]
        $after = if ($insertAt -lt $indexLines.Count) { $indexLines[$insertAt..($indexLines.Count - 1)] } else { @() }
        ($before + $newRow + $after) | Set-Content -Path $IndexFile -Encoding utf8
        if (-not $Quiet) {
            Write-Host "  skills/index.md: row inserted"
        }
    }
}

if (Test-Path $AuditScript) {
    & powershell -NoProfile -ExecutionPolicy Bypass -File $AuditScript -SkillName $SkillName -Quiet
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAIL: audit-skill validation failed for $SkillName"
        exit 1
    }
}

if (-not $Quiet) {
    Write-Host ""
    Write-Host "Skill created: $SkillName"
    Write-Host "  Directory: $SkillDir"
    Write-Host "  SKILL.md:  $SkillFile"
    Write-Host "  scripts/:  $ScriptsDir"
    Write-Host ""
    Write-Host "Next: edit SKILL.md placeholders, then run:"
    Write-Host "  skills/audit-project/scripts/audit.ps1 -Root `"$Root`" -Quiet"
}

exit 0
