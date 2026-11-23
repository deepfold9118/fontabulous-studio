# Builds Fontabulous Studio (release) and wraps it in the installer.
#
# Output in dist\: Fontabulous-Studio-Setup-<version>.exe (the installer),
# fontabulous-studio.exe (the program on its own, runs without installing) and a
# .sha256 file for each.
#
# The build remaps the source and Cargo paths so the executables do not contain
# the folders they were built in (Rust panic messages embed them).
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$sep = [char]0x1f

Push-Location $root
try {
    # Remap the folder both as written here and as Cargo spells it (Windows paths are
    # case-insensitive, but the remapping is not).
    $cargoRoot = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).workspace_root
    $flags = @("--remap-path-prefix=$cargoHome=/cargo")
    foreach ($p in @($root, $cargoRoot) | Select-Object -Unique) { $flags += "--remap-path-prefix=$p=/fontabulous-studio" }
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join $sep

    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'App build failed' }
    Push-Location installer
    try {
        cargo build --release --target-dir ..\target
        if ($LASTEXITCODE -ne 0) { throw 'Installer build failed' }
    } finally { Pop-Location }

    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
    New-Item -ItemType Directory -Force dist | Out-Null
    $outputs = @(
        @{ From = 'target\release\fontabulous-studio-setup.exe'; To = "Fontabulous-Studio-Setup-$version.exe" },
        @{ From = 'target\release\fontabulous_studio.exe'; To = 'fontabulous-studio.exe' }
    )
    foreach ($o in $outputs) {
        $dest = Join-Path 'dist' $o.To
        Copy-Item $o.From $dest -Force
        $hash = (Get-FileHash $dest -Algorithm SHA256).Hash.ToLower()
        Set-Content -Path "$dest.sha256" -Value "$hash  $($o.To)" -NoNewline -Encoding ascii
        Write-Host "$dest  $hash"
    }
} finally {
    Pop-Location
    Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue
}
