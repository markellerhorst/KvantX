<#
.SYNOPSIS
  One-time, idempotent rename of msQuant to KvantX in the text files of the repository.
.DESCRIPTION
  Run from the repository root (look first with -DryRun):
      powershell -ExecutionPolicy Bypass -File tools\rename-to-kvantx.ps1 -DryRun
      powershell -ExecutionPolicy Bypass -File tools\rename-to-kvantx.ps1
  Replaces  msquant -> kvantx,  msQuant -> KvantX,  msQuant-default -> KvantX-default  and the project
  extension  .quant -> .kvantx  in *.rs, *.md, *.toml, *.yml, *.txt and .gitignore.
  Kept unchanged: the icon file names img\msQuant-ico.* (until a KvantX icon exists), docs\roadmap.md
  (history), LICENSE, Cargo.lock (cargo updates it) and everything in target, .git, testData, test-data,
  img and tools\ThermoRawFileParser*.
#>
param([switch]$DryRun)
$ErrorActionPreference = 'Stop'
$root = (Get-Location).Path
if (-not (Test-Path (Join-Path $root 'Cargo.toml'))) { throw 'Run this script from the repository root (Cargo.toml not found).' }

$skipDir  = @('target', '.git', 'test-data', 'testData', 'img', 'KvantX-default', 'msQuant-default')
$skipFile = @('roadmap.md', 'LICENSE', 'rename-to-kvantx.ps1')
$exts     = @('.rs', '.md', '.toml', '.yml', '.yaml', '.txt', '.gitignore')
$mark     = [string][char]1 + 'ICO' + [string][char]1
$utf8     = New-Object System.Text.UTF8Encoding($false)

function Convert-Text([string]$t) {
    $t = $t.Replace('msQuant-ico', $mark)                  # protect the icon file names
    $t = $t.Replace('msQuant-default', 'KvantX-default')
    $t = $t.Replace('msquant', 'kvantx')
    $t = $t.Replace('msQuant', 'KvantX')
    $t = [regex]::Replace($t, '\.quant\b', '.kvantx')
    $t = $t.Replace('["quant"]', '["kvantx"]').Replace('with_extension("quant")', 'with_extension("kvantx")')
    $t = $t.Replace('lcmsq_audit', 'kvantx_audit')
    return $t.Replace($mark, 'msQuant-ico')
}

$changed = New-Object System.Collections.Generic.List[string]
$files = Get-ChildItem -Path $root -Recurse -File -Force
foreach ($f in $files) {
    $rel = $f.FullName.Substring($root.Length + 1)
    $parts = $rel.Split([char[]]@('\', '/'))
    $dirParts = @()
    if ($parts.Length -gt 1) { $dirParts = $parts[0..($parts.Length - 2)] }
    $skip = $false
    foreach ($d in $dirParts) { if ($skipDir -contains $d -or $d -like 'ThermoRawFileParser*') { $skip = $true } }
    if ($skip -or ($skipFile -contains $f.Name) -or ($exts -notcontains $f.Extension)) { continue }
    $text = [System.IO.File]::ReadAllText($f.FullName)
    $new = Convert-Text $text
    if ($new -ne $text) {
        $changed.Add($rel)
        if (-not $DryRun) { [System.IO.File]::WriteAllText($f.FullName, $new, $utf8) }
    }
}
$mode = if ($DryRun) { 'would change' } else { 'changed' }
Write-Host ("{0} {1} files:" -f $mode, $changed.Count)
$changed | ForEach-Object { Write-Host "  $_" }

# what is left (expected: only the icon file names, docs\roadmap.md and LICENSE)
if (-not $DryRun) {
    $left = foreach ($f in $files) {
        $rel = $f.FullName.Substring($root.Length + 1)
        if ($rel -match '^(target|\.git|test-data|testData|img)[\\/]' -or $rel -like 'tools*ThermoRawFileParser*') { continue }
        if ($exts -notcontains $f.Extension) { continue }
        Select-String -Path $f.FullName -Pattern 'msquant', 'msQuant' -CaseSensitive -SimpleMatch |
            Where-Object { $_.Line -notmatch 'msQuant-ico' }
    }
    Write-Host ''
    Write-Host 'Remaining mentions (expected: docs\roadmap.md and LICENSE only):'
    $left | ForEach-Object { Write-Host ("  {0}:{1}" -f $_.Path.Substring($root.Length + 1), $_.LineNumber) }
}
