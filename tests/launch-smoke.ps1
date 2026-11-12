<#
.SYNOPSIS
  Launch smoke test: starts the real InputSwitch.exe and checks it comes up properly.

.DESCRIPTION
  Uses a throwaway settings folder (INPUTSWITCH_CONFIG_DIR), so it never touches your real
  settings and can run while your own copy of InputSwitch is running. Checks that:
    1. --diagnose runs and writes its report,
    2. --settings opens the settings window and it draws its content,
    3. no other window (such as an error dialog) appears,
    4. the app is still running afterwards.
  It never switches a monitor input: no hotkeys are set up and nothing is clicked.

.EXAMPLE
  pwsh tests/launch-smoke.ps1 -Exe bin\Release\net10.0-windows\InputSwitch.exe -OutDir smoke-out
#>
param(
    [Parameter(Mandatory)] [string] $Exe,
    [string] $OutDir = (Join-Path ([IO.Path]::GetTempPath()) "InputSwitch-smoke-out"),
    [string] $Capture,             # optional path to WindowCapture.exe, for a screenshot
    [int] $TimeoutSeconds = 30
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]

$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force $OutDir | Out-Null
$configDir = Join-Path ([IO.Path]::GetTempPath()) "InputSwitch-smoke-$([guid]::NewGuid().ToString('N'))"
$env:INPUTSWITCH_CONFIG_DIR = $configDir

$failures = [System.Collections.Generic.List[string]]::new()
function Check([bool] $ok, [string] $what) {
    if ($ok) { Write-Host "  PASS  $what" } else { Write-Host "  FAIL  $what"; $failures.Add($what) }
}

function Windows-Of([int] $processId) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $processId)
    @($A::RootElement.FindAll($Scope::Children, $cond))
}

$app = $null
try {
    # 1. Diagnostics report (read-only).
    $report = Join-Path $OutDir 'diagnostics.txt'
    $diag = Start-Process $Exe -ArgumentList '--diagnose', "`"$report`"" -PassThru -Wait
    Check ($diag.ExitCode -eq 0) "--diagnose exits cleanly (exit code $($diag.ExitCode))"
    Check ((Test-Path $report) -and ((Get-Content $report -Raw) -match 'Version:')) "--diagnose writes a report with the version"

    # 2. Settings window opens and renders.
    $app = Start-Process $Exe -ArgumentList '--settings' -PassThru
    $window = $null
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (-not $window -and (Get-Date) -lt $deadline -and -not $app.HasExited) {
        Start-Sleep -Milliseconds 500
        $window = Windows-Of $app.Id | Where-Object { $_.Current.Name -eq 'InputSwitch' } | Select-Object -First 1
    }
    Check (-not $app.HasExited) "app is running after launch"
    Check ($null -ne $window) "settings window appears within $TimeoutSeconds s"

    if ($window) {
        Start-Sleep -Seconds 2   # let first-run detection settle
        # Match by name on any element: WPF reports labels as Text, but plain Win32 labels come
        # through as Pane in PowerShell 7's UI Automation client.
        $texts = $window.FindAll($Scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
            ForEach-Object { $_.Current.Name }
        Check ($texts -contains 'Configurations') "settings window shows its content"
        Check ([bool]($texts | Where-Object { $_ -match '^Version \d+\.\d+\.\d+' })) "settings window shows the version"

        # 3. No unexpected dialogs (an error would appear as a second window).
        $others = @(Windows-Of $app.Id | Where-Object { $_.Current.NativeWindowHandle -ne $window.Current.NativeWindowHandle })
        Check ($others.Count -eq 0) "no error or other dialogs ($(@($others | ForEach-Object { $_.Current.Name }) -join ', '))"

        if ($Capture -and (Test-Path $Capture)) {
            & $Capture --pid $app.Id --title InputSwitch --out (Join-Path $OutDir 'settings-window.png')
            if ($LASTEXITCODE -ne 0) { Write-Host "  note  screenshot not taken (capture unavailable here)" }
        }
    }

    # 4. Still alive at the end.
    $app.Refresh()
    Check (-not $app.HasExited) "app is still running at the end"
}
finally {
    if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id -Force }
    Remove-Item Env:INPUTSWITCH_CONFIG_DIR -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $configDir -ErrorAction SilentlyContinue
}

Write-Host ""
if ($failures.Count) {
    # Put the diagnostic report in the log, so a failed CI run explains itself.
    if (Test-Path $report) {
        Write-Host "---- diagnostic report ----"
        Get-Content $report | ForEach-Object { Write-Host "  $_" }
        Write-Host "---------------------------"
    }
    Write-Host "$($failures.Count) launch check(s) FAILED."
    exit 1
}
Write-Host "All launch checks passed."
