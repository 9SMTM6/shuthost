# Windows CI test for the host agent update installers.
# Installs an older release via the PowerShell enduser installer, then updates it.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Wait-ForAgentReady {
    param([int]$TimeoutSeconds = 30)

    Write-Host "Waiting for shuthost_host_agent.exe or host_agent.exe to start (up to $TimeoutSeconds seconds)..."
    for ($i = 1; $i -le $TimeoutSeconds; $i++) {
        $proc = Get-Process -Name shuthost_host_agent -ErrorAction SilentlyContinue
        if (-not $proc) {
            $proc = Get-Process -Name host_agent -ErrorAction SilentlyContinue
        }
        if ($proc) {
            Write-Host 'Host agent process is running.'
            return
        }
        Start-Sleep -Seconds 1
    }
    throw 'Host agent did not become ready within timeout.'
}

function Wait-ForCoordinatorReady {
    param([int]$TimeoutSeconds = 30)

    Write-Host "Waiting for coordinator to become ready on http://localhost:8080/login..."
    for ($i = 1; $i -le $TimeoutSeconds; $i++) {
        try {
            Invoke-WebRequest -Uri 'http://localhost:8080/login' -UseBasicParsing -ErrorAction Stop | Out-Null
            Write-Host 'Coordinator is ready.'
            return
        } catch {
            Start-Sleep -Seconds 1
        }
    }
    throw 'Coordinator did not become ready within timeout.'
}

function Copy-ScriptToTemp {
    param([string]$SourcePath)
    $temp = [System.IO.Path]::GetTempFileName()
    $dest = [System.IO.Path]::ChangeExtension($temp, [System.IO.Path]::GetFileName($SourcePath))
    Remove-Item -Path $temp -Force -ErrorAction SilentlyContinue
    Copy-Item -Path $SourcePath -Destination $dest -Force
    return $dest
}

$ErrorActionPreference = 'Stop'

$HostInstallerSource = '.\scripts\enduser_installers\host_agent.ps1'
$CoordinatorInstallerSource = '.\scripts\coordinator_installers\host_agent.ps1'
$HostInstaller = Copy-ScriptToTemp $HostInstallerSource
$CoordinatorInstaller = Copy-ScriptToTemp $CoordinatorInstallerSource
$CoordinatorBinary = '.\shuthost_coordinator.exe'
$TargetTag = '1.6.4'

Write-Host 'Starting Windows installer update test'
Write-Host "Using temporary enduser installer script: $HostInstaller"
Write-Host "Using temporary coordinator installer script: $CoordinatorInstaller"

Write-Host "Installing old release $TargetTag via enduser installer"
& pwsh -NoProfile -ExecutionPolicy Bypass -File $HostInstaller -Tag $TargetTag

Wait-ForAgentReady

Write-Host 'Updating host agent using enduser installer update mode'
& pwsh -NoProfile -ExecutionPolicy Bypass -File $HostInstaller -Update

Wait-ForAgentReady

Write-Host 'Starting local coordinator as service'
& $CoordinatorBinary install $env:USERNAME --port 8080 --bind 127.0.0.1

Wait-ForCoordinatorReady

Write-Host 'Updating host agent using coordinator installer update mode'
& pwsh -NoProfile -ExecutionPolicy Bypass -File $CoordinatorInstaller http://127.0.0.1:8080 -Update

Wait-ForAgentReady

Write-Host 'Verifying host agent process remains running after update'
Get-Process -Name shuthost_host_agent -ErrorAction Stop | Out-Null

Write-Host 'Windows installer update test completed successfully!'
