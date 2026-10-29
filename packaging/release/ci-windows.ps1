# T09 orchestration only; T08 owns building and packaging the Windows payload.
param([Parameter(Mandatory=$true)][string]$SourceSha)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot/../..").Path
$pins = Get-Content "$PSScriptRoot/inputs.json" -Raw | ConvertFrom-Json
$work = Join-Path $env:RUNNER_TEMP 'osu-radio-windows'
$materials = Join-Path $env:RUNNER_TEMP 'windows-materials'
python -B "$PSScriptRoot/stage.py" check $SourceSha
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
python -B "$PSScriptRoot/stage.py" materials $materials
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$installer = Join-Path $env:RUNNER_TEMP 'innosetup.exe'
Invoke-WebRequest $pins.windows.inno_url -OutFile $installer
if ((Get-FileHash $installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne $pins.windows.inno_sha256) {
    throw 'Inno Setup installer checksum mismatch'
}
$signature = Get-AuthenticodeSignature $installer
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Pyrsys B\.V\.') {
    throw 'Inno Setup installer signature invalid'
}
$inno = Join-Path $env:RUNNER_TEMP 'inno'
$process = Start-Process -FilePath $installer -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DIR=$inno" -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Inno Setup installation failed: $($process.ExitCode)" }
$iscc = Join-Path $inno 'ISCC.exe'
$isccHash = (Get-FileHash $iscc -Algorithm SHA256).Hash.ToLowerInvariant()
$redist = Join-Path $env:VCToolsRedistDir 'x64/Microsoft.VC143.CRT'
# Keep SDK selection outside the clean source checkout.
$sdkConfig = Join-Path (Split-Path $repo -Parent) 'global.json'
@{ sdk = @{ version = $pins.windows.dotnet_sdk; rollForward = 'disable' } } | ConvertTo-Json | Set-Content $sdkConfig
python -B "$repo/packaging/windows/build.py" --qt $env:QT_ROOT_DIR --redist $redist --iscc $iscc --materials $materials --work $work --qt-version $pins.windows.qt --dotnet-sdk $pins.windows.dotnet_sdk --msvc-version $pins.windows.msvc --windows-sdk $pins.windows.windows_sdk --iscc-sha256 $isccHash
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
python -B "$PSScriptRoot/stage.py" check $SourceSha
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# Only this small, explicit directory is uploaded; no Cargo caches or environment dumps.
$handoff = Join-Path $env:RUNNER_TEMP 'windows-handoff'
New-Item -ItemType Directory $handoff | Out-Null
Copy-Item "$work/output" $handoff -Recurse
Copy-Item "$work/stage/licenses" $handoff -Recurse
Copy-Item "$work/stage/BUILD-INFO.txt", "$work/payload.sha256", "$work/setup.sha256" $handoff
