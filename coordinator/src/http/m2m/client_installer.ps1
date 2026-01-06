param(
    [Parameter(Position=0)]
    [string]$RemoteUrl = "http://localhost:8080",

    [Parameter(Position=1)]
    [string]$ClientId
)

# Default values
$installDir = "$env:USERPROFILE\bin"
$remoteUrl = $RemoteUrl

# Ensure the installation directory exists
if (-not (Test-Path $installDir)) {
    Write-Host "Creating installation directory: $installDir"
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

# Check if the installation directory is in PATH
$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
$machinePath = [Environment]::GetEnvironmentVariable("PATH", "Machine")
$fullPath = "$userPath;$machinePath"

if ($fullPath -notlike "*$installDir*") {
    Write-Host "Warning: $installDir is not in your PATH."
    Write-Host "You may need to add it to your PATH to use the installed script easily."
    Write-Host "To do so, add the following to your PATH environment variable:"
    Write-Host "$installDir"
}

# Word lists for generating readable client IDs
$adjectives = @("red", "blue", "swift", "calm", "bold", "wise", "kind", "brave")
$nouns = @("fox", "bird", "wolf", "bear", "lion", "deer", "hawk", "eagle")

# Generate a random client ID using word lists
$randomAdjective = $adjectives | Get-Random
$randomNoun = $nouns | Get-Random
$randomPart = "${randomAdjective}_${randomNoun}"

if ($ClientId) {
    $baseId = $ClientId
} else {
    $baseId = $randomPart
}

$subdomain = ($(hostname) -split '\.')[0]
$ClientId = "${subdomain}_${baseId}"

$clientScriptName = "shuthost_client_${baseId}.ps1"

################## Boring setup complete ------------- Interesting stuff is starting here

# Download the client script template
Write-Host "Downloading client script template..."
Write-Verbose "Remote URL: $remoteUrl"
Write-Verbose "Client ID: $ClientId"

$templateUrl = "$remoteUrl/download/shuthost_client.ps1"
$tempTemplatePath = "$env:TEMP\$clientScriptName.tmpl"

& curl.exe --compressed -L --fail-with-body -o $tempTemplatePath $templateUrl

# Generate a random shared secret
$secretBytes = New-Object byte[] 16
[System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($secretBytes)
$sharedSecret = [BitConverter]::ToString($secretBytes).Replace('-', '').ToLower()

# Replace placeholders in the script
$templateContent = Get-Content $tempTemplatePath -Raw
$customizedContent = $templateContent -replace '\{client_id\}', $ClientId `
                                       -replace '\{shared_secret\}', $sharedSecret `
                                       -replace '\{embedded_remote_url\}', $remoteUrl

# Save the customized script
$finalPath = Join-Path $installDir $clientScriptName
$customizedContent | Out-File -FilePath $finalPath -Encoding UTF8

# Clean up temp file
Remove-Item $tempTemplatePath -Force

################## Aaand done -----------------------------------------------------

# Print the configuration line for the coordinator
Write-Host "Installation complete!"
Write-Host "Add the following line to your coordinator config under [clients]:"
Write-Host ""
Write-Host "`"$ClientId`" = { shared_secret = `"$sharedSecret`" }"
Write-Host ""
Write-Host "Afterwards you can use the client script with the following command:"
Write-Host "$finalPath <take|release> <host> [remote_url] [-Async]"

# Clean up installer
$installerPath = $MyInvocation.MyCommand.Path
if (Test-Path $installerPath) {
    Remove-Item $installerPath -Force
}