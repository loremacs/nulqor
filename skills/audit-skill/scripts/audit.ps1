# audit.ps1 -- Nulqor skill structural linter
# Platform: Windows PowerShell 5.1+
#
# Runs F/M/C/I/Q checks against every skill in skills/.
# Exits 0 if no FAILs; exits 1 if any FAIL exists.
#
# Usage:
#   .\scripts\audit.ps1                           # audit all skills
#   .\scripts\audit.ps1 -SkillName create-skill   # audit one skill
#   .\scripts\audit.ps1 -SkillPath skills/<name>    # legacy alias for one skill
#   .\scripts\audit.ps1 -Quiet                    # suppress PASS lines
#   .\scripts\audit.ps1 -Json                     # machine-readable JSON

param(
    [string]$SkillName = "",
    [string]$SkillPath = "",
    [switch]$Quiet,
    [switch]$Json
)

if ($SkillPath -ne "") {
    $resolved = (Resolve-Path $SkillPath).Path
    $SkillName = Split-Path $resolved -Leaf
}

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

$ScriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Path
$SkillsRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
$IndexFile  = Join-Path $SkillsRoot "index.md"

$ALLOWED_SUBDIRS = @("scripts","references","assets","evals","data")
$SKIP_SKILLS     = @()
$ALLOWED_FM_KEYS = @("name","description","license","compatibility","metadata","allowed-tools")
$SEMVER_RE       = '^[0-9]+\.[0-9]+\.[0-9]+$'
$NAME_RE         = '^[a-z][a-z0-9\-]*[a-z0-9]$'
$SCRIPT_EXTS     = @(".ps1",".sh",".bat",".cmd",".py",".rb")

# ---------------------------------------------------------------------------
# Security pattern tables (S-category checks)
# CRITICAL → S1 FAIL. HIGH → S2 WARN.
# ---------------------------------------------------------------------------

$SEC_CRITICAL = @(
    [PSCustomObject]@{ Code="S1"; Pattern='(?i)(Invoke-Expression|iex)\s*[\(\$\|]';  Desc="iex/Invoke-Expression with variable or pipe (download-and-execute)" }    # nosec
    [PSCustomObject]@{ Code="S1"; Pattern='(?i)curl[^|]*\|\s*(bash|sh)';             Desc="curl | bash/sh (download-and-execute)" }                                  # nosec
    [PSCustomObject]@{ Code="S1"; Pattern='(?i)wget[^|]*\|\s*(bash|sh)';             Desc="wget | bash/sh (download-and-execute)" }                                  # nosec
    [PSCustomObject]@{ Code="S1"; Pattern='(?i)iwr[^|]*\|\s*iex';                    Desc="Invoke-WebRequest | iex (download-and-execute)" }                         # nosec
    [PSCustomObject]@{ Code="S1"; Pattern='(?i)\[Convert\]::FromBase64String';       Desc="Base64 decode (potential obfuscated payload)" }                           # nosec
    [PSCustomObject]@{ Code="S1"; Pattern='(?i)\beval\b.+\$';                        Desc="eval with variable input (dynamic code execution)" }                      # nosec
)

$SEC_HIGH = @(
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)(Invoke-WebRequest|Invoke-RestMethod|curl|wget)\s+[''"]?https?://(?!localhost|127\.0\.0\.1)'; Desc="External HTTP call (exfiltration risk)" }  # nosec
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)Remove-Item.+-Recurse.+-Force\s+(C:\\|~|/)';  Desc="Destructive Remove-Item outside repo" }                                                    # nosec
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)rm\s+-rf\s+(\/|~|\$HOME|C:\\)';               Desc="rm -rf on root, home, or system drive" }                                                   # nosec
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)[''"]?~[/\\]\.ssh[/\\]';                       Desc=".ssh/ path access (credential exposure)" }                                                # nosec
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)\$env:WINDIR|\$env:SYSTEMROOT|C:\\Windows\\';  Desc="Write to system directory" }                                                              # nosec
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)HKLM:|HKCU:\\SOFTWARE\\';                      Desc="Registry write (persistence risk)" }                                                      # nosec
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)Set-ExecutionPolicy';                           Desc="Execution policy change" }                                                                # nosec
    [PSCustomObject]@{ Code="S2"; Pattern='(?i)Disable-\w*(Defender|Firewall|UAC|Antivirus)'; Desc="Security control disabled" }                                                              # nosec
)

# ---------------------------------------------------------------------------
# Helper: build a finding object
# ---------------------------------------------------------------------------

function New-Finding([string]$Code, [string]$Level, [string]$Message) {
    [PSCustomObject]@{ Code = $Code; Level = $Level; Message = $Message }
}

function Get-WorstLevel([object[]]$Findings) {
    if ($Findings | Where-Object { $_.Level -eq "FAIL" }) { return "FAIL" }
    if ($Findings | Where-Object { $_.Level -eq "WARN" }) { return "WARN" }
    return "PASS"
}

function Get-MetadataFields([string[]]$Lines, [int]$FmEnd) {
    $meta = @{}
    $metaIdx = -1
    for ($i = $FmEnd + 1; $i -lt $Lines.Count; $i++) {
        if ($Lines[$i] -match '^## Metadata\s*$') { $metaIdx = $i; break }
    }
    if ($metaIdx -lt 0) { return $meta }

    $inBlock = $false
    for ($i = $metaIdx + 1; $i -lt $Lines.Count; $i++) {
        if ($Lines[$i] -match '^##\s') { break }
        if ($Lines[$i] -match '^```') {
            if (-not $inBlock) { $inBlock = $true; continue }
            else { break }
        }
        if ($Lines[$i] -match '^([\w\-]+):\s*(.*)$') {
            $meta[$Matches[1]] = $Matches[2].Trim()
        }
    }
    return $meta
}

# ---------------------------------------------------------------------------
# Audit one skill directory
# ---------------------------------------------------------------------------

function Invoke-SkillAudit([System.IO.DirectoryInfo]$Dir) {
    $skill     = $Dir.Name
    $skillMd   = Join-Path $Dir.FullName "SKILL.md"
    $legacyMd  = Join-Path $Dir.FullName "skill.md"
    $scriptsD  = Join-Path $Dir.FullName "scripts"
    $f         = [System.Collections.ArrayList]@()

    # F1 -- SKILL.md must exist (open standard filename)
    if (-not (Test-Path $skillMd)) {
        if (Test-Path $legacyMd) {
            [void]$f.Add((New-Finding "F1" "FAIL" "skill.md found; rename to SKILL.md (host standard)"))
            $skillMd = $legacyMd
        } else {
            [void]$f.Add((New-Finding "F1" "FAIL" "SKILL.md does not exist"))
            return [PSCustomObject]@{ Skill=$skill; Status="FAIL"; Findings=@($f) }
        }
    }

    # F2 -- scripts/ must exist
    if (-not (Test-Path $scriptsD)) {
        [void]$f.Add((New-Finding "F2" "FAIL" "scripts/ directory is missing"))
    }

    # F3 -- no unexpected subdirectories
    $extra = Get-ChildItem $Dir.FullName -Directory | Where-Object { $ALLOWED_SUBDIRS -notcontains $_.Name }
    foreach ($x in $extra) {
        [void]$f.Add((New-Finding "F3" "WARN" "Unexpected directory: $($x.Name)"))
    }

    # Read file
    $raw       = [System.IO.File]::ReadAllText($skillMd) -replace "`r",""
    $lines     = $raw -split "`n"
    $lineCount = $lines.Count

    # M1 -- must open with ---
    if ($lines[0] -ne "---") {
        [void]$f.Add((New-Finding "M1" "FAIL" "Line 1 is not '---' (got: $($lines[0]))"))
    }

    # Find frontmatter end
    $fmEnd = -1
    $limit = [Math]::Min($lineCount, 30)
    for ($i = 1; $i -lt $limit; $i++) {
        if ($lines[$i] -eq "---") { $fmEnd = $i; break }
    }

    if ($fmEnd -lt 0) {
        [void]$f.Add((New-Finding "MF1" "FAIL" "Frontmatter closing '---' not found in first 30 lines"))
        return [PSCustomObject]@{ Skill=$skill; Status="FAIL"; Findings=@($f) }
    }

    # Parse frontmatter (supports description: > and description: | folded blocks)
    $fm = @{}
    $fi = 1
    while ($fi -lt $fmEnd) {
        if ($lines[$fi] -match '^([\w\-]+):\s*(.*)$') {
            $fKey = $Matches[1]
            $fVal = $Matches[2].Trim()
            if ($fVal -eq '>' -or $fVal -eq '|') {
                $folded = [System.Collections.ArrayList]@()
                $fi++
                while ($fi -lt $fmEnd) {
                    if ($lines[$fi] -match '^[\w\-]+:\s*') { $fi--; break }
                    $t = $lines[$fi].Trim()
                    if ($t -ne '') { [void]$folded.Add($t) }
                    $fi++
                }
                $fm[$fKey] = ($folded -join ' ').Trim()
            } else {
                $fm[$fKey] = $fVal
            }
        }
        $fi++
    }

    # M2/M3/M4/M5 -- name
    if (-not $fm.ContainsKey("name")) {
        [void]$f.Add((New-Finding "M2" "FAIL" "Frontmatter missing 'name:' field"))
    } else {
        $n = $fm["name"]
        if ($n -ne $skill) {
            [void]$f.Add((New-Finding "M3" "FAIL" "name '$n' does not match directory '$skill'"))
        }
        if ($n.Length -gt 1 -and $n -notmatch $NAME_RE) {
            [void]$f.Add((New-Finding "M4" "FAIL" "name '$n' is not valid lowercase-hyphenated"))
        }
        if ($n.Length -gt 64) {
            [void]$f.Add((New-Finding "M5" "FAIL" "name length $($n.Length) exceeds 64 chars"))
        }
    }

    # M6/M7/Q1 -- description
    if (-not $fm.ContainsKey("description") -or $fm["description"] -eq "") {
        [void]$f.Add((New-Finding "M6" "FAIL" "Frontmatter missing or empty 'description:' field"))
    } else {
        $desc = $fm["description"]
        $dl   = $desc.ToLower()
        $hasTrigger = ($dl -match "use when") -or ($dl -match "apply when") -or
                      ($dl -match "when ") -or ($dl -match "before ") -or ($dl -match "after ")
        if (-not $hasTrigger) {
            [void]$f.Add((New-Finding "M7" "WARN" "description may lack a trigger condition"))
        }
        if ($desc.Length -gt 200) {
            [void]$f.Add((New-Finding "Q1" "WARN" "description is $($desc.Length) chars (recommend <200)"))
        }
    }

    # M12 -- host fields must not live in frontmatter
    foreach ($key in $fm.Keys) {
        if ($ALLOWED_FM_KEYS -notcontains $key) {
            [void]$f.Add((New-Finding "M12" "WARN" "Frontmatter field '${key}' should move to ## Metadata section"))
        }
    }
    if ($fm.ContainsKey("version") -or $fm.ContainsKey("topics") -or $fm.ContainsKey("platform")) {
        [void]$f.Add((New-Finding "M12" "WARN" "version/topics/platform in frontmatter -- move to ## Metadata"))
    }

    $meta = Get-MetadataFields $lines $fmEnd

    # MD1 -- Metadata section
    if ($meta.Count -eq 0 -and $raw -notmatch '(?m)^## Metadata\s*$') {
        [void]$f.Add((New-Finding "MD1" "FAIL" "## Metadata section missing"))
    }

    # M8 -- version in Metadata
    if (-not $meta.ContainsKey("version") -or $meta["version"] -eq "") {
        [void]$f.Add((New-Finding "M8" "FAIL" "## Metadata missing version"))
    } elseif ($meta["version"] -notmatch $SEMVER_RE) {
        [void]$f.Add((New-Finding "M8" "FAIL" "version '$($meta['version'])' is not valid semver"))
    }

    # M9 -- topics in Metadata
    if (-not $meta.ContainsKey("topics") -or $meta["topics"] -eq "") {
        [void]$f.Add((New-Finding "M9" "FAIL" "## Metadata missing topics"))
    }

    # M10 -- platform in Metadata
    $plat = ""
    if ($meta.ContainsKey("platform")) { $plat = $meta["platform"].ToLower() }
    if ($plat -eq "") {
        [void]$f.Add((New-Finding "M10" "FAIL" "## Metadata missing platform"))
    }

    # M11 -- OS prefix must match platform
    if ($skill -match '^(win|mac|linux)-') {
        $pfx = $Matches[1]
        $expected = switch ($pfx) { "win" { "windows" } "mac" { "macos" } "linux" { "linux" } }
        if ($plat -ne "" -and $plat -ne $expected -and $plat -notmatch $expected) {
            [void]$f.Add((New-Finding "M11" "FAIL" "prefix '$pfx-' but platform is '$plat'"))
        }
    }

    # C1 -- Contract section
    $contractIdx = -1
    for ($i = $fmEnd + 1; $i -lt $lineCount; $i++) {
        if ($lines[$i] -match '^## Contract\s*$') { $contractIdx = $i; break }
    }

    if ($contractIdx -lt 0) {
        [void]$f.Add((New-Finding "C1" "FAIL" "'## Contract' section not found"))
    } else {
        $bStart = -1
        $searchEnd = [Math]::Min($lineCount, $contractIdx + 10)
        for ($i = $contractIdx + 1; $i -lt $searchEnd; $i++) {
            if ($lines[$i] -match '^```') { $bStart = $i; break }
        }

        if ($bStart -lt 0) {
            [void]$f.Add((New-Finding "C2" "FAIL" "No code block found after '## Contract'"))
        } else {
            $bEnd = -1
            for ($i = $bStart + 1; $i -lt $lineCount; $i++) {
                if ($lines[$i] -match '^```') { $bEnd = $i; break }
            }

            if ($bEnd -lt 0) {
                [void]$f.Add((New-Finding "C2" "FAIL" "Contract code block not closed"))
            } else {
                $block = ($lines[($bStart + 1)..($bEnd - 1)]) -join "`n"

                if ($block -match '(?m)^name:\s+') {
                    [void]$f.Add((New-Finding "C6" "WARN" "Contract contains name: (use frontmatter only)"))
                }
                if ($block -match '(?m)^version:\s+') {
                    [void]$f.Add((New-Finding "C6" "WARN" "Contract contains version: (use ## Metadata)"))
                }
                if ($block -match '(?m)^topics:\s+') {
                    [void]$f.Add((New-Finding "C6" "WARN" "Contract contains topics: (use ## Metadata)"))
                }

                if ($block -notmatch '(?m)^when:') {
                    [void]$f.Add((New-Finding "C3" "FAIL" "Contract missing 'when:' field"))
                }
                if ($block -notmatch '(?m)^outputs:') {
                    [void]$f.Add((New-Finding "C4" "FAIL" "Contract missing 'outputs:' field"))
                }
                if ($block -notmatch '(?m)^side-effects:') {
                    [void]$f.Add((New-Finding "C5" "FAIL" "Contract missing 'side-effects:' field"))
                }
                if ($block -notmatch '(?m)^validation:') {
                    [void]$f.Add((New-Finding "C7" "WARN" "Contract missing 'validation:' field"))
                }
            }
        }
    }

    # I1/I2 -- index.md registration
    if (-not (Test-Path $IndexFile)) {
        [void]$f.Add((New-Finding "I1" "WARN" "index.md not found at $IndexFile"))
    } else {
        $indexRaw  = [System.IO.File]::ReadAllText($IndexFile) -replace "`r",""
        $needle    = "``" + $skill + "``"
        $indexRows = @(($indexRaw -split "`n") | Where-Object { $_.Contains($needle) })
        if ($indexRows.Count -eq 0) {
            [void]$f.Add((New-Finding "I1" "FAIL" "No index.md row for '$skill'"))
        } else {
            $cols = @(($indexRows[0] -split '\|') | Where-Object { $_.Trim() -ne "" })
            if ($cols.Count -lt 2) {
                [void]$f.Add((New-Finding "I2" "FAIL" "Index row has <2 columns (expected Skill | Purpose)"))
            }
        }
    }

    # P1/P2 -- platform script companions (from Metadata platform)
    if ($plat -eq "all" -and (Test-Path $scriptsD)) {
        $hasPs1 = @(Get-ChildItem $scriptsD -Filter "*.ps1" -File -ErrorAction SilentlyContinue).Count -gt 0
        $hasSh  = @(Get-ChildItem $scriptsD -Filter "*.sh" -File -ErrorAction SilentlyContinue).Count -gt 0
        $hasPy  = @(Get-ChildItem $scriptsD -Filter "*.py" -File -ErrorAction SilentlyContinue).Count -gt 0
        if ($hasPs1 -and -not $hasSh -and -not $hasPy) {
            [void]$f.Add((New-Finding "P1" "WARN" "platform: all has .ps1 but no .sh or .py companion"))
        }
        if ($hasSh -and -not $hasPs1 -and -not $hasPy) {
            [void]$f.Add((New-Finding "P2" "WARN" "platform: all has .sh but no .ps1 or .py companion"))
        }
    }

    # Q2 -- line count
    if ($lineCount -gt 500) {
        [void]$f.Add((New-Finding "Q2" "WARN" "SKILL.md is $lineCount lines (recommend <500)"))
    }

    # Q3 -- Verification section
    if ($raw -notmatch '(?m)^## Verification') {
        [void]$f.Add((New-Finding "Q3" "WARN" "No '## Verification' section"))
    }

    # Q4 -- Problem or When to use
    if ($raw -notmatch '(?m)^## (Problem|When to use)') {
        [void]$f.Add((New-Finding "Q4" "WARN" "No '## Problem' or '## When to use' section"))
    }

    # Q5 -- scripts referenced in body
    if (Test-Path $scriptsD) {
        $sFiles = @(Get-ChildItem $scriptsD -File -ErrorAction SilentlyContinue)
        if ($sFiles.Count -gt 0) {
            $anyRef = $false
            foreach ($sf in $sFiles) {
                if ($raw.Contains($sf.Name)) { $anyRef = $true; break }
            }
            if (-not $anyRef) {
                [void]$f.Add((New-Finding "Q5" "WARN" "scripts/ has files but none referenced by name in body"))
            }
        } elseif (-not $raw.Contains("intentionally empty")) {
            [void]$f.Add((New-Finding "Q5" "WARN" "scripts/ is empty -- add note or a script"))
        }
    }

    # S1/S2 -- security scan of script files
    if (Test-Path $scriptsD) {
        $secFiles = @(Get-ChildItem $scriptsD -File -ErrorAction SilentlyContinue |
            Where-Object { $SCRIPT_EXTS -contains $_.Extension.ToLower() })
        foreach ($sf in $secFiles) {
            $sfContent = [System.IO.File]::ReadAllText($sf.FullName) -replace "`r",""
            $sfLines   = $sfContent -split "`n"
            for ($ln = 0; $ln -lt $sfLines.Count; $ln++) {
                $line = $sfLines[$ln]
                if ($line.TrimStart().StartsWith("#")) { continue }  # full comment line
                if ($line -match '#\s*nosec\b') { continue }         # nosec annotation
                foreach ($p in $SEC_CRITICAL) {
                    if ($line -match $p.Pattern) {
                        [void]$f.Add((New-Finding $p.Code "FAIL" "$($sf.Name):$($ln+1) CRITICAL: $($p.Desc)"))
                    }
                }
                foreach ($p in $SEC_HIGH) {
                    if ($line -match $p.Pattern) {
                        [void]$f.Add((New-Finding $p.Code "WARN" "$($sf.Name):$($ln+1) HIGH: $($p.Desc)"))
                    }
                }
            }
        }
    }

    $findings = @($f)
    $status   = Get-WorstLevel $findings
    return [PSCustomObject]@{ Skill=$skill; Status=$status; Findings=$findings }
}

# ---------------------------------------------------------------------------
# Collect directories
# ---------------------------------------------------------------------------

if (-not (Test-Path $SkillsRoot)) {
    Write-Host "ERROR: skills root not found: $SkillsRoot" -ForegroundColor Red
    exit 1
}

$skillDirs = @(
    Get-ChildItem $SkillsRoot -Directory | Sort-Object Name |
        Where-Object { $_.Name -notmatch '\s' -and $_.Name -notmatch ' copy$' }
)

if ($SkillName -ne "") {
    $skillDirs = @($skillDirs | Where-Object { $_.Name -eq $SkillName })
    if ($skillDirs.Count -eq 0) {
        Write-Host "ERROR: skill '$SkillName' not found in $SkillsRoot" -ForegroundColor Red
        exit 1
    }
}

# ---------------------------------------------------------------------------
# Run audits
# ---------------------------------------------------------------------------

$allResults = [System.Collections.ArrayList]@()

foreach ($dir in $skillDirs) {
    $s = $dir.Name

    if ($SKIP_SKILLS -contains $s) {
        [void]$allResults.Add([PSCustomObject]@{ Skill=$s; Status="SKIP"; Findings=@() })
        if (-not $Quiet -and -not $Json) {
            Write-Host "`n[$s]" -ForegroundColor DarkGray
            Write-Host "  [SKIP] Superseded" -ForegroundColor DarkGray
        }
        continue
    }

    $result = Invoke-SkillAudit $dir
    [void]$allResults.Add($result)

    if (-not $Json) {
        $showSkill = (-not $Quiet) -or ($result.Status -ne "PASS")
        if ($showSkill) {
            $hColor = switch ($result.Status) {
                "PASS" { "Green"  }
                "WARN" { "Yellow" }
                "FAIL" { "Red"    }
                default { "White" }
            }
            Write-Host "`n[$s]  $($result.Status)" -ForegroundColor $hColor
            foreach ($fi in $result.Findings) {
                $fColor = switch ($fi.Level) { "FAIL"{"Red"} "WARN"{"Yellow"} default{"Green"} }
                if (-not $Quiet -or $fi.Level -ne "PASS") {
                    Write-Host "  [$($fi.Level)] $($fi.Code): $($fi.Message)" -ForegroundColor $fColor
                }
            }
        }
    }
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

$passCount = @($allResults | Where-Object { $_.Status -eq "PASS" }).Count
$warnCount = @($allResults | Where-Object { $_.Status -eq "WARN" }).Count
$failCount = @($allResults | Where-Object { $_.Status -eq "FAIL" }).Count
$skipCount = @($allResults | Where-Object { $_.Status -eq "SKIP" }).Count

$failItems = @($allResults | ForEach-Object {
    $sk = $_.Skill
    $_.Findings | Where-Object { $_.Level -eq "FAIL" } |
        ForEach-Object { [PSCustomObject]@{ Skill=$sk; Code=$_.Code; Message=$_.Message } }
})
$warnItems = @($allResults | ForEach-Object {
    $sk = $_.Skill
    $_.Findings | Where-Object { $_.Level -eq "WARN" } |
        ForEach-Object { [PSCustomObject]@{ Skill=$sk; Code=$_.Code; Message=$_.Message } }
})

if ($Json) {
    [PSCustomObject]@{
        date    = (Get-Date -Format "yyyy-MM-dd")
        summary = [PSCustomObject]@{
            total = $allResults.Count
            pass  = $passCount
            warn  = $warnCount
            fail  = $failCount
            skip  = $skipCount
        }
        results = $allResults
    } | ConvertTo-Json -Depth 10
} else {
    Write-Host ""
    Write-Host "=======================================" -ForegroundColor DarkGray
    Write-Host " SKILL AUDIT  $(Get-Date -Format 'yyyy-MM-dd')" -ForegroundColor White
    Write-Host "=======================================" -ForegroundColor DarkGray
    Write-Host " Total : $($allResults.Count)" -ForegroundColor White
    Write-Host " PASS  : $passCount"           -ForegroundColor Green
    Write-Host " WARN  : $warnCount"           -ForegroundColor Yellow
    Write-Host " FAIL  : $failCount"           -ForegroundColor Red
    Write-Host " SKIP  : $skipCount"           -ForegroundColor DarkGray
    Write-Host "=======================================" -ForegroundColor DarkGray

    if ($failItems.Count -gt 0) {
        Write-Host ""
        Write-Host "PRIORITY FIXES (FAIL -- must resolve before phase gate):" -ForegroundColor Red
        foreach ($fi in $failItems) {
            Write-Host "  $($fi.Skill)  [$($fi.Code)]  $($fi.Message)" -ForegroundColor Red
        }
    }
    if ($warnItems.Count -gt 0) {
        Write-Host ""
        Write-Host "WARNINGS (resolve before marking skill stable):" -ForegroundColor Yellow
        foreach ($wi in $warnItems) {
            Write-Host "  $($wi.Skill)  [$($wi.Code)]  $($wi.Message)" -ForegroundColor Yellow
        }
    }
    if ($failCount -eq 0 -and $warnCount -eq 0) {
        Write-Host ""
        Write-Host " All skills pass structural audit." -ForegroundColor Green
    }
}

if ($failCount -gt 0) { exit 1 } else { exit 0 }
