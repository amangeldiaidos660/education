param(
    [Parameter(Mandatory=$true)][string]$ProgramKeypair,
    [string]$Keygen = 'solana-keygen'
)
$ErrorActionPreference = 'Stop'
if (!(Test-Path -LiteralPath $ProgramKeypair -PathType Leaf)) { throw 'Program keypair does not exist.' }
$programAddress = (& $Keygen pubkey $ProgramKeypair | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $programAddress -notmatch '^[1-9A-HJ-NP-Za-km-z]{32,44}$') { throw 'Could not read the program public address.' }
$sourcePath = Join-Path $PSScriptRoot 'programs/sozbricks_rewards/src/lib.rs'
$anchorPath = Join-Path $PSScriptRoot 'Anchor.toml'
$source = [System.IO.File]::ReadAllText($sourcePath)
$anchor = [System.IO.File]::ReadAllText($anchorPath)
$source = [regex]::Replace($source, 'declare_id!\("[1-9A-HJ-NP-Za-km-z]+"\);', "declare_id!(`"$programAddress`");")
$anchor = [regex]::Replace($anchor, 'sozbricks_rewards = "[1-9A-HJ-NP-Za-km-z]+"', "sozbricks_rewards = `"$programAddress`"")
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($sourcePath, $source, $utf8)
[System.IO.File]::WriteAllText($anchorPath, $anchor, $utf8)
Write-Output "Program ID set to $programAddress. Rebuild SBF before deploying."
