<#
.SYNOPSIS
Build a portable Bandix GUI directory and ZIP archive for Windows.

.PARAMETER Architecture
Build for the current machine (native), x64, or arm64. The default is native.

.PARAMETER OutputDir
Write packages to this directory instead of target\dist.

.EXAMPLE
.\scripts\package-windows.ps1

.EXAMPLE
.\scripts\package-windows.ps1 -Architecture arm64 -OutputDir artifacts
#>
#Requires -Version 5.1
[CmdletBinding()]
param(
    [ValidateSet("native", "x64", "arm64")]
    [string]$Architecture = "native",

    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

try {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw "This package must be built on Windows."
    }

    foreach ($tool in @("cargo", "rustc", "git")) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "Missing build tool: $tool"
        }
    }

    $nativeMachine = $env:PROCESSOR_ARCHITEW6432
    if ([string]::IsNullOrWhiteSpace($nativeMachine)) {
        $nativeMachine = $env:PROCESSOR_ARCHITECTURE
    }

    if ($Architecture -eq "native") {
        switch ($nativeMachine.ToUpperInvariant()) {
            "AMD64" { $Architecture = "x64" }
            "ARM64" { $Architecture = "arm64" }
            default { throw "Unsupported native architecture: $nativeMachine" }
        }
    }

    switch ($Architecture) {
        "x64" {
            $rustTarget = "x86_64-pc-windows-msvc"
            $packageArch = "x86_64"
        }
        "arm64" {
            $rustTarget = "aarch64-pc-windows-msvc"
            $packageArch = "arm64"
        }
    }

    $targetLibDir = (& rustc --print target-libdir --target $rustTarget | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $targetLibDir -PathType Container)) {
        throw "Missing Rust target '$rustTarget'. Install it with: rustup target add $rustTarget"
    }

    $metadataText = (& cargo metadata --no-deps --format-version 1 | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to read Cargo metadata."
    }
    $metadata = $metadataText | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq "bandix-gui" } | Select-Object -First 1
    if ($null -eq $package) {
        throw "Cargo metadata does not contain the bandix-gui package."
    }

    $version = $package.version
    $targetDirectory = [IO.Path]::GetFullPath([string]$metadata.target_directory)
    if ([string]::IsNullOrWhiteSpace($OutputDir)) {
        $out = Join-Path $targetDirectory "dist"
    } elseif ([IO.Path]::IsPathRooted($OutputDir)) {
        $out = [IO.Path]::GetFullPath($OutputDir)
    } else {
        $out = [IO.Path]::GetFullPath((Join-Path $root $OutputDir))
    }

    New-Item -ItemType Directory -Force -Path $out | Out-Null
    $work = Join-Path $out (".bandix-windows-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $work | Out-Null

    try {
        Write-Host "Building release binary for $rustTarget..."
        & cargo build --release --locked --target $rustTarget
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo build failed."
        }

        $binary = Join-Path $targetDirectory "$rustTarget\release\bandix-gui.exe"
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
            throw "Release binary was not created: $binary"
        }

        $name = "bandix-gui-$version-windows-$packageArch"
        $stage = Join-Path $work $name
        New-Item -ItemType Directory -Path $stage | Out-Null
        Copy-Item -LiteralPath $binary -Destination (Join-Path $stage "bandix-gui.exe")
        Copy-Item -LiteralPath (Join-Path $root "assets\icons\bandix-256.png") `
            -Destination (Join-Path $stage "bandix-gui.png")

        $readme = @"
Bandix GUI $version - Windows $packageArch

Run bandix-gui.exe to start the application. Keep the files in this directory
together if you move the application. Windows 10 or later is recommended.

Connect to your Bandix service from Settings. Automatic default-gateway
discovery is not currently available on Windows, so enter the server address
manually, for example http://192.168.1.1:8686.
"@
        Set-Content -LiteralPath (Join-Path $stage "README.txt") -Value $readme -Encoding UTF8

        $rustcVersion = (& rustc --version | Out-String).Trim()
        $sourceCommit = (& git rev-parse HEAD | Out-String).Trim()
        $sourceDirty = ((& git status --porcelain | Out-String).Trim().Length -gt 0)
        $lockHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $root "Cargo.lock")).Hash.ToLowerInvariant()
        $buildInfo = [ordered]@{
            name = "bandix-gui"
            version = $version
            os = "windows"
            architecture = $packageArch
            rust_target = $rustTarget
            format = "zip"
            built_at_utc = [DateTime]::UtcNow.ToString("o")
            windows_version = [Environment]::OSVersion.VersionString
            rustc = $rustcVersion
            source_commit = $sourceCommit
            source_dirty = $sourceDirty
            cargo_lock_sha256 = $lockHash
        }
        $buildInfo | ConvertTo-Json -Depth 4 | Set-Content `
            -LiteralPath (Join-Path $stage "BUILD-INFO.json") -Encoding UTF8

        $temporaryZip = Join-Path $work "$name.zip"
        Compress-Archive -Path $stage -DestinationPath $temporaryZip -CompressionLevel Optimal

        $publishedDirectory = Join-Path $out $name
        $publishedZip = Join-Path $out "$name.zip"
        $publishedChecksum = Join-Path $out "$name.sha256"
        if (Test-Path -LiteralPath $publishedDirectory) {
            Remove-Item -LiteralPath $publishedDirectory -Recurse -Force
        }
        if (Test-Path -LiteralPath $publishedZip) {
            Remove-Item -LiteralPath $publishedZip -Force
        }
        Move-Item -LiteralPath $stage -Destination $publishedDirectory
        Move-Item -LiteralPath $temporaryZip -Destination $publishedZip

        $zipHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $publishedZip).Hash.ToLowerInvariant()
        Set-Content -LiteralPath $publishedChecksum -Encoding ASCII -Value "$zipHash  $name.zip"

        Write-Host ""
        Write-Host "Windows $packageArch package saved to $out"
        Get-Item -LiteralPath $publishedZip, $publishedChecksum | Format-Table Name, Length -AutoSize
    } finally {
        if (Test-Path -LiteralPath $work) {
            Remove-Item -LiteralPath $work -Recurse -Force
        }
    }
} finally {
    Pop-Location
}
