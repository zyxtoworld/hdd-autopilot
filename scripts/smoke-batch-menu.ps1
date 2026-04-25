param(
    [switch]$Build = $true,
    [switch]$SkipBalance,
    [switch]$SkipCheckin,
    [switch]$SkipScratch,
    [switch]$SkipSheepMatch
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$distExe = Join-Path $root 'dist\hdd-win-x64.exe'
$artifactDir = Join-Path $root '.tmp-smoke-script'
$returnMarker = '已返回上一级菜单。'
$mainMenuPrompt = '请输入选项 (1/2/3):'
$batchMenuPrompt = '请输入选项 (1/2/3/4):'
$featureMenuPrompt = '请输入选项 (1/2/3/4/5/6):'
$balanceDoneMarker = '全部账号余额查询完成。若要返回上一级菜单，请按 ESC。'
$checkinDoneMarker = '全部账号签到完成。若要返回上一级菜单，请按 ESC。'
$scratchDoneMarker = '自动随机刮刮乐处理完成。若要返回上一级菜单，请按 ESC。'
$sheepDoneMarker = '自动羊了个羊处理完成。若要返回上一级菜单，请按 ESC。'
if (!(Test-Path $artifactDir)) {
    New-Item -ItemType Directory -Path $artifactDir | Out-Null
}

function Ensure-Build {
    if ($Build -or !(Test-Path $distExe)) {
        Push-Location $root
        try {
            & go build -o $distExe .\cmd\hdd
            if ($LASTEXITCODE -ne 0) {
                throw 'go build failed'
            }
        } finally {
            Pop-Location
        }
    }
}


function Start-InteractiveCli {
    $proc = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/u', '/c', ('cd /d "' + $root + '" && dist\hdd-win-x64.exe > ".tmp-smoke-script\session.log" 2>&1')) -PassThru -WindowStyle Maximized
    Start-Sleep -Milliseconds 1200
    $wshell = New-Object -ComObject WScript.Shell
    $null = $wshell.AppActivate($proc.Id)
    Start-Sleep -Milliseconds 300
    Wait-ForText $mainMenuPrompt 15 | Out-Null
    return @{ Process = $proc; Shell = $wshell }
}

function Stop-InteractiveCli {
    param($Session)
    if ($null -eq $Session) {
        return
    }
    $proc = $Session.Process
    if ($proc -and -not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force -Confirm:$false
    }
}

function Send-Keys {
    param($Session, [string]$Keys, [int]$DelayMs = 350)
    $null = $Session.Shell.AppActivate($Session.Process.Id)
    Start-Sleep -Milliseconds 150
    $Session.Shell.SendKeys($Keys)
    Start-Sleep -Milliseconds $DelayMs
}

function Read-SessionLog {
    $path = Join-Path $artifactDir 'session.log'
    if (!(Test-Path $path)) {
        return ''
    }
    return Get-Content -Path $path -Encoding utf8 -Raw -ErrorAction SilentlyContinue
}

function Wait-ForText {
    param([string]$Expected, [int]$TimeoutSeconds = 120)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $text = Read-SessionLog
        if ($text.Contains($Expected)) {
            return $text
        }
        Start-Sleep -Milliseconds 500
    }
    throw "did not find expected text: $Expected"
}

function Wait-ForProcessExit {
    param($Session, [int]$TimeoutSeconds = 15)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline -and -not $Session.Process.HasExited) {
        Start-Sleep -Milliseconds 200
    }
    if (-not $Session.Process.HasExited) {
        throw 'process did not exit in time'
    }
}

function Enter-BatchFeatureMenu {
    param($Session, [string]$FeatureChoice)
    Send-Keys $Session '2{ENTER}'
    Wait-ForText $batchMenuPrompt 15 | Out-Null
    Send-Keys $Session '2{ENTER}'
    Wait-ForText $featureMenuPrompt 15 | Out-Null
    Send-Keys $Session ($FeatureChoice + '{ENTER}')
}

function Return-ToFeatureMenu {
    param($Session)
    Send-Keys $Session '{ESC}' 900
    Wait-ForText $returnMarker 15 | Out-Null
}

function Exit-ProgramFromFeatureMenu {
    param($Session)
    Send-Keys $Session '5{ENTER}'
    Send-Keys $Session '3{ENTER}'
    Send-Keys $Session '3{ENTER}'
    Wait-ForProcessExit $Session
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
    foreach ($path in $Paths) {
        $old = 0
        if ($Before.ContainsKey($path)) {
            $old = $Before[$path]
        }
        $new = 0
        if (Test-Path $path) {
            $new = (Get-Item $path).Length
        }
        if ($new -le $old) {
            throw "expected file to grow: $path ($old -> $new)"
        }
    }
}


function Run-BalanceSmoke {
    Remove-Item -Path (Join-Path $artifactDir 'session.log') -ErrorAction SilentlyContinue
    $session = Start-InteractiveCli
    try {
        Enter-BatchFeatureMenu $session '1'
        Wait-ForText $balanceDoneMarker 120 | Out-Null
        Return-ToFeatureMenu $session
        $text = Get-Content -Path (Join-Path $artifactDir 'session.log') -Encoding utf8 -Raw
        if (!$text.Contains($returnMarker)) {
            throw 'balance flow did not print return marker'
        }
        Exit-ProgramFromFeatureMenu $session
        Write-Output 'balance: ok'
    } finally {
        Stop-InteractiveCli $session
    }
}

function Run-CheckinSmoke {
    $sharedLog = Join-Path $root 'log\checkin\checkin.log'
    $before = Get-FileSizes @($sharedLog)
    Remove-Item -Path (Join-Path $artifactDir 'session.log') -ErrorAction SilentlyContinue
    $session = Start-InteractiveCli
    try {
        Enter-BatchFeatureMenu $session '2'
        Wait-ForText $checkinDoneMarker 120 | Out-Null
        Return-ToFeatureMenu $session
        $text = Get-Content -Path (Join-Path $artifactDir 'session.log') -Encoding utf8 -Raw
        if (!$text.Contains($returnMarker)) {
            throw 'checkin flow did not print return marker'
        }
        Assert-FileGrowth $before @($sharedLog)
        Exit-ProgramFromFeatureMenu $session
        Write-Output 'checkin: ok'
    } finally {
        Stop-InteractiveCli $session
    }
}

function Run-ScratchSmoke {
    $logPaths = @(
        (Join-Path $root 'log\scratch\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'log\scratch\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'log\scratch\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'log\scratch\demo_at_example.com.log')
    )
    $before = Get-FileSizes $logPaths
    Remove-Item -Path (Join-Path $artifactDir 'session.log') -ErrorAction SilentlyContinue
    $session = Start-InteractiveCli
    try {
        Enter-BatchFeatureMenu $session '3'
        Wait-ForText $scratchDoneMarker 120 | Out-Null
        $deadline = (Get-Date).AddSeconds(120)
        while ((Get-Date) -lt $deadline) {
            $allDone = $true
            foreach ($path in $logPaths) {
                $old = $before[$path]
                $new = 0
                if (Test-Path $path) {
                    $new = (Get-Item $path).Length
                }
                if ($new -le $old) {
                    $allDone = $false
                    break
                }
            }
            if ($allDone) {
                Start-Sleep -Milliseconds 1800
                break
            }
            Start-Sleep -Milliseconds 500
        }
        Return-ToFeatureMenu $session
        $text = Get-Content -Path (Join-Path $artifactDir 'session.log') -Encoding utf8 -Raw
        if (!$text.Contains($returnMarker)) {
            throw 'scratch flow did not print return marker'
        }
        Assert-FileGrowth $before $logPaths
        Exit-ProgramFromFeatureMenu $session
        Write-Output 'scratch: ok'
    } finally {
        Stop-InteractiveCli $session
    }
}

function Run-SheepMatchSmoke {
    $logPaths = @(
        (Join-Path $root 'log\sheep-match\aiuser001_at_fuckwall.eu.org.log'),
        (Join-Path $root 'log\sheep-match\aiuser002_at_fuckwall.eu.org.log'),
        (Join-Path $root 'log\sheep-match\aiuser003_at_fuckwall.eu.org.log'),
        (Join-Path $root 'log\sheep-match\demo_at_example.com.log')
    )
    $before = Get-FileSizes $logPaths
    Remove-Item -Path (Join-Path $artifactDir 'session.log') -ErrorAction SilentlyContinue
    $session = Start-InteractiveCli
    try {
        Enter-BatchFeatureMenu $session '4'
        Wait-ForText $sheepDoneMarker 120 | Out-Null
        Return-ToFeatureMenu $session
        $text = Get-Content -Path (Join-Path $artifactDir 'session.log') -Encoding utf8 -Raw
        if (!$text.Contains($returnMarker)) {
            throw 'sheep-match flow did not print return marker'
        }
        Assert-FileGrowth $before $logPaths
        Exit-ProgramFromFeatureMenu $session
        Write-Output 'sheep-match: ok'
    } finally {
        Stop-InteractiveCli $session
    }
}

Ensure-Build

if (-not $SkipBalance) {
    Run-BalanceSmoke
}
if (-not $SkipCheckin) {
    Run-CheckinSmoke
}
if (-not $SkipScratch) {
    Run-ScratchSmoke
}
if (-not $SkipSheepMatch) {
    Run-SheepMatchSmoke
}

Write-Output 'batch smoke: done'
