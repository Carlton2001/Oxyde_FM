[CmdletBinding()]
param([switch]$Offline)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$destination = Join-Path $projectRoot 'licenses/dependencies'
$temporary = Join-Path $destination 'THIRD-PARTY-NOTICES.md.tmp'
Push-Location -LiteralPath $projectRoot
try {
    $arguments = @('about', 'generate', '--locked', '--fail', '--output-file', $temporary, 'scripts/third-party-notices.hbs')
    if ($Offline) { $arguments += '--offline' }
    & cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw 'License generation failed. Install cargo-about 0.9.2 with --locked --features cli and resolve its diagnostics.' }
    if (-not (Test-Path -LiteralPath $temporary) -or (Get-Item -LiteralPath $temporary).Length -eq 0) { throw 'Empty third-party notices.' }
    # NOTICE/COPYRIGHT files are not necessarily recognized as license texts by cargo-about.
    $metadata = & cargo metadata --locked --offline --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cannot collect upstream notices.' }
    $supplement = [Text.StringBuilder]::new()
    [void]$supplement.Append("`n# Additional upstream notices`n`nOriginal NOTICE, COPYRIGHT and AUTHORS files, identified by package and source path.`n")
    foreach ($package in (($metadata | ConvertFrom-Json).packages | Sort-Object name, version, source)) {
        if (-not $package.source) { continue }
        $root = Split-Path -Parent $package.manifest_path
        foreach ($file in (Get-ChildItem -LiteralPath $root -Recurse -File | Where-Object { $_.Name -match '^(NOTICE|NOTICES|COPYRIGHT|AUTHORS)([-_.].*)?$' } | Sort-Object FullName)) {
            $relative = $file.FullName.Substring($root.Length + 1).Replace('\', '/')
            $contents = [IO.File]::ReadAllText($file.FullName)
            # Use a fence longer than any run in the upstream text, preserving it verbatim.
            $longest = 0
            foreach ($run in [regex]::Matches($contents, '~+')) { $longest = [Math]::Max($longest, $run.Length) }
            $fence = '~' * [Math]::Max(3, $longest + 1)
            [void]$supplement.Append("`n## $($package.name) $($package.version) — $relative`n`nSource: $($package.source)`n`n${fence}text`n")
            [void]$supplement.Append($contents)
            [void]$supplement.Append("`n$fence`n")
        }
    }
    [IO.File]::AppendAllText($temporary, $supplement.ToString(), [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporary -Destination (Join-Path $destination 'THIRD-PARTY-NOTICES.md') -Force
    $upstream = Join-Path $destination 'upstream'
    if (Test-Path -LiteralPath $upstream) {
        if ([IO.Path]::GetFullPath($upstream) -ne (Join-Path $projectRoot 'licenses/dependencies/upstream')) { throw 'Unexpected upstream notice path.' }
        Remove-Item -LiteralPath $upstream -Recurse -Force
    }
    $inputs = @('Cargo.lock', 'Cargo.toml', 'about.toml', 'scripts/third-party-notices.hbs', 'scripts/update-third-party-licenses.ps1')
    $hashes = [ordered]@{}
    foreach ($inputPath in $inputs) { $hashes[$inputPath] = (Get-FileHash -LiteralPath $inputPath -Algorithm SHA256).Hash }
    $hashes['licenses/dependencies/THIRD-PARTY-NOTICES.md'] = (Get-FileHash -LiteralPath (Join-Path $destination 'THIRD-PARTY-NOTICES.md') -Algorithm SHA256).Hash
    [IO.File]::WriteAllText((Join-Path $destination 'inputs.json'), ($hashes | ConvertTo-Json) + "`n", [Text.UTF8Encoding]::new($false))
} finally {
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
    Pop-Location
}
