param([ValidateSet('dev', 'verify', 'build', 'preview')][string]$Task = 'dev')
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'Visual Studio C++ tools are required' }
$installation = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installation) { throw 'Visual Studio C++ tools are required' }
& (Join-Path $installation 'Common7\Tools\Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
Push-Location (Join-Path $PSScriptRoot '..\..')
try {
    $lockPath = Join-Path (Get-Location) '.build.lock'
    $lockToken = "windows:${PID}:$([guid]::NewGuid())"
    try {
        $lock = [System.IO.File]::Open($lockPath, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
        try {
            $content = [System.Text.Encoding]::UTF8.GetBytes($lockToken)
            $lock.Write($content, 0, $content.Length)
        } finally {
            $lock.Dispose()
        }
    } catch [System.IO.IOException] {
        throw 'Another build is active; stop it before starting a new task'
    }
    $env:CODEX_STATUS_BUILD_LOCK = $lockToken
    $env:CI = 'true'
    & pnpm.cmd install --frozen-lockfile --trust-lockfile --offline
    if ($LASTEXITCODE -ne 0) { & pnpm.cmd install --frozen-lockfile --trust-lockfile }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & pnpm.cmd $Task
    exit $LASTEXITCODE
} finally {
    Remove-Item Env:CODEX_STATUS_BUILD_LOCK -ErrorAction SilentlyContinue
    if ($lockPath -and (Test-Path $lockPath) -and ([System.IO.File]::ReadAllText($lockPath) -eq $lockToken)) {
        Remove-Item $lockPath -Force
    }
    Pop-Location
}
