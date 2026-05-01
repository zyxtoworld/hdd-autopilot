param(
    [switch]$Build = $true,
    [switch]$SkipCheckin,
    [switch]$SkipMemory = $true,
    [switch]$SkipPuzzle15 = $true,
    [switch]$SkipPuzzle2048,
    [switch]$SkipScratch = $true,
    [switch]$SkipSheepMatch,
    [switch]$SkipSudoku = $true
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [Console]::OutputEncoding

$root = Split-Path -Parent $PSScriptRoot
$distExe = Join-Path $root 'dist\hdd-autopilot-win-x64.exe'
$artifactDir = Join-Path $root '.tmp-smoke-batch'
$returnMarker = '已返回上一级菜单。'
$checkinDoneMarker = '全部账号签到完成。'
$memoryDoneMarker = '自动记忆翻牌处理完成。'
$puzzle15DoneMarker = '自动华容道处理完成。'
$puzzle2048DoneMarker = '自动谜题2048处理完成。'
$scratchDoneMarker = '自动随机刮刮乐处理完成。'
$sheepDoneMarker = '自动羊了个羊处理完成。'
$sudokuDoneMarker = '自动数独处理完成。'
$freeDoneMarker = '全自动完成所有白嫖玩法。'

if (!(Test-Path $artifactDir)) {
    New-Item -ItemType Directory -Path $artifactDir | Out-Null
}

function Ensure-Build {
    if ($Build -or !(Test-Path $distExe)) {
        Push-Location $root
        try {
            & .\scripts\build-win-x64.bat
            if ($LASTEXITCODE -ne 0) {
                throw 'Rust build failed'
            }
        } finally {
            Pop-Location
        }
    }
}

function Get-FileSizes {
    param([string[]]$Paths)
    $sizes = @{}
    foreach ($path in $Paths) {
        if (Test-Path $path) {
            $sizes[$path] = (Get-Item $path).Length
        } else {
            $sizes[$path] = 0
        }
    }
    return $sizes
}

function Assert-FileGrowth {
    param($Before, [string[]]$Paths)
    $grew = $false
    foreach ($path in $Paths) {
        $old = 0
        if ($Before.ContainsKey($path)) {
            $old = $Before[$path]
        }
        $new = 0
        if (Test-Path $path) {
            $new = (Get-Item $path).Length
        }
        if ($new -gt $old) {
            $grew = $true
        }
    }
    if (-not $grew) {
        throw "expected at least one file to grow: $($Paths -join ', ')"
    }
}

function Invoke-HeadlessFlow {
    param(
        [string]$Name,
        [string[]]$Inputs,
        [string]$DoneMarker,
        [string[]]$LogPaths = @()
    )

    $sessionLog = Join-Path $artifactDir ($Name + '.log')
    $inputFile = Join-Path $artifactDir ($Name + '.input.txt')
    $before = Get-FileSizes $LogPaths
    $inputText = (($Inputs -join "`r`n") + "`r`n")
    [System.IO.File]::WriteAllText($inputFile, $inputText, [System.Text.UTF8Encoding]::new($false))

    Push-Location $root
    try {
        & cmd.exe /d /c "chcp 65001>nul && set HDD_SMOKE_AUTO_RETURN=1 && dist\hdd-autopilot-win-x64.exe < `"$inputFile`" > `"$sessionLog`" 2>&1"
        if ($LASTEXITCODE -ne 0) {
            throw "$Name flow exited with code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    $text = Get-Content -Path $sessionLog -Encoding utf8 -Raw
    if (!$text.Contains($DoneMarker)) {
        throw "$Name flow missing done marker"
    }
    if (!$text.Contains($returnMarker)) {
        throw "$Name flow missing return marker"
    }
    if ($LogPaths.Count -gt 0) {
        Assert-FileGrowth $before $LogPaths
    }
    Write-Output ($Name + ': ok')
}

Ensure-Build

if (-not $SkipCheckin) {
    Invoke-HeadlessFlow -Name 'checkin' -Inputs @('2', '2', '1', '2', '8', '3', '3') -DoneMarker $checkinDoneMarker -LogPaths @(
        (Join-Path $root 'var\log\checkin\checkin.log')
    )
}
if (-not $SkipScratch) {
    Invoke-HeadlessFlow -Name 'scratch' -Inputs @('2', '2', '2', '1', '2', '3', '3') -DoneMarker $scratchDoneMarker -LogPaths @(
        (Join-Path $root 'var\log\scratch\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\scratch\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\scratch\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\scratch\demo_at_example.com.log')
    )
}
if (-not $SkipPuzzle2048) {
    Invoke-HeadlessFlow -Name 'puzzle2048' -Inputs @('2', '2', '1', '4', '8', '3', '3') -DoneMarker $puzzle2048DoneMarker -LogPaths @(
        (Join-Path $root 'var\log\puzzle_2048\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\puzzle_2048\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\puzzle_2048\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\puzzle_2048\demo_at_example.com.log')
    )
}

if (-not $SkipMemory) {
    Invoke-HeadlessFlow -Name 'memory' -Inputs @('2', '2', '1', '5', '8', '3', '3') -DoneMarker $memoryDoneMarker -LogPaths @(
        (Join-Path $root 'var\log\memory\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\memory\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\memory\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\memory\demo_at_example.com.log')
    )
}

if (-not $SkipPuzzle15) {
    Invoke-HeadlessFlow -Name 'puzzle15' -Inputs @('2', '2', '1', '6', '8', '3', '3') -DoneMarker $puzzle15DoneMarker -LogPaths @(
        (Join-Path $root 'var\log\puzzle_15\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\puzzle_15\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\puzzle_15\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\puzzle_15\demo_at_example.com.log')
    )
}

if (-not $SkipSudoku) {
    Invoke-HeadlessFlow -Name 'sudoku' -Inputs @('2', '2', '1', '7', '8', '3', '3') -DoneMarker $sudokuDoneMarker -LogPaths @(
        (Join-Path $root 'var\log\sudoku\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\sudoku\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\sudoku\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\sudoku\demo_at_example.com.log')
    )
}

if (-not $SkipSheepMatch) {
    Invoke-HeadlessFlow -Name 'sheep-match' -Inputs @('2', '2', '1', '3', '8', '3', '3') -DoneMarker $sheepDoneMarker -LogPaths @(
        (Join-Path $root 'var\log\sheepmatch\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\sheepmatch\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\sheepmatch\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'var\log\sheepmatch\demo_at_example.com.log')
    )
}

Invoke-HeadlessFlow -Name 'free-auto' -Inputs @('2', '2', '1', '1', '8', '3', '3') -DoneMarker $freeDoneMarker -LogPaths @(
    (Join-Path $root 'var\log\checkin\checkin.log'),
    (Join-Path $root 'var\log\sheepmatch\aiuser001_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\sheepmatch\aiuser002_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\sheepmatch\aiuser003_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\sheepmatch\demo_at_example.com.log'),
    (Join-Path $root 'var\log\puzzle_2048\aiuser001_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\puzzle_2048\aiuser002_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\puzzle_2048\aiuser003_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\puzzle_2048\demo_at_example.com.log'),
    (Join-Path $root 'var\log\memory\aiuser001_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\memory\aiuser002_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\memory\aiuser003_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\memory\demo_at_example.com.log'),
    (Join-Path $root 'var\log\puzzle_15\aiuser001_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\puzzle_15\aiuser002_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\puzzle_15\aiuser003_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\puzzle_15\demo_at_example.com.log'),
    (Join-Path $root 'var\log\sudoku\aiuser001_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\sudoku\aiuser002_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\sudoku\aiuser003_at_fuckwall.eu.org.log'),
    (Join-Path $root 'var\log\sudoku\demo_at_example.com.log')
)

Write-Output '批量菜单烟测完成。'
