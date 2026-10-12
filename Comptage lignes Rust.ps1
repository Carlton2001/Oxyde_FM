$excludedDirs = @("target", "reference_zed", "tests", ".perso", ".git")

Get-ChildItem -Path . -Filter "*.rs" -Recurse -File |
    Where-Object {
        $relativePath = $_.FullName.Substring((Get-Location).Path.Length).TrimStart('\')
        $parts = $relativePath -split '[\\/]'
        -not ($parts | Select-Object -SkipLast 1 | Where-Object { $_ -in $excludedDirs })
    } |
    ForEach-Object {
        [PSCustomObject]@{
            Lignes = @(Get-Content -LiteralPath $_.FullName).Count
            Fichier = $_.FullName
        }
    } |
    Sort-Object Lignes -Descending |
    Format-Table -AutoSize -Wrap
