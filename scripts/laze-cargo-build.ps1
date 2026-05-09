param(
    [Parameter(Mandatory = $true)]
    [string]$ManifestDir,

    [Parameter(Mandatory = $true)]
    [string]$Profile,

    [Parameter(Mandatory = $true)]
    [string]$Target,

    [Parameter(Mandatory = $true)]
    [string]$TargetDir,

    [Parameter(Mandatory = $true)]
    [string]$Board,

    [string]$Features,
    [string]$OpenOcdArgs,
    [string]$ScriptsDir,
    [string]$ExecutorStacksize,
    [string]$IsrStacksize,
    [string]$Swi,
    [string]$DefmtLog,
    [string]$LogLevel,
    [string]$HeapSize,
    [string]$Cc,
    [string]$Cflags,
    [string]$PeersYml
)

$ErrorActionPreference = 'Stop'

$manifestPath = Join-Path $ManifestDir 'Cargo.toml'
if (!(Test-Path $manifestPath)) {
    throw "Cargo manifest not found: $manifestPath"
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$absoluteTargetDir = if ([System.IO.Path]::IsPathRooted($TargetDir)) {
    $TargetDir
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repoRoot $TargetDir))
}

$cargoHomeBase = 'C:\cg'
if (!(Test-Path $cargoHomeBase)) {
    New-Item -ItemType Directory -Force -Path $cargoHomeBase | Out-Null
}

$cargoHome = Join-Path $cargoHomeBase '427'
New-Item -ItemType Directory -Force -Path $cargoHome | Out-Null

$targetEnvPrefix = 'CARGO_TARGET_' + ($Target.ToUpper() -replace '-', '_')

[System.Environment]::SetEnvironmentVariable('CONFIG_BOARD', $Board, 'Process')
[System.Environment]::SetEnvironmentVariable('CARGO_BUILD_TARGET', $Target, 'Process')
[System.Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $absoluteTargetDir, 'Process')
[System.Environment]::SetEnvironmentVariable('CARGO_HOME', $cargoHome, 'Process')
[System.Environment]::SetEnvironmentVariable('CARGO_NET_GIT_FETCH_WITH_CLI', 'true', 'Process')

if ($OpenOcdArgs) {
    [System.Environment]::SetEnvironmentVariable('OPENOCD_ARGS', $OpenOcdArgs, 'Process')
}
if ($ScriptsDir) {
    [System.Environment]::SetEnvironmentVariable('SCRIPTS', $ScriptsDir, 'Process')
}
if ($ExecutorStacksize) {
    [System.Environment]::SetEnvironmentVariable('CONFIG_EXECUTOR_STACKSIZE', $ExecutorStacksize, 'Process')
}
if ($IsrStacksize) {
    [System.Environment]::SetEnvironmentVariable('CONFIG_ISR_STACKSIZE', $IsrStacksize, 'Process')
}
if ($Swi) {
    [System.Environment]::SetEnvironmentVariable('CONFIG_SWI', $Swi, 'Process')
}
if ($DefmtLog) {
    [System.Environment]::SetEnvironmentVariable('DEFMT_LOG', $DefmtLog, 'Process')
}
if ($LogLevel) {
    [System.Environment]::SetEnvironmentVariable('LOG_LEVEL', $LogLevel, 'Process')
}
if ($HeapSize) {
    [System.Environment]::SetEnvironmentVariable('CONFIG_HEAPSIZE', $HeapSize, 'Process')
}
if ($Cc) {
    [System.Environment]::SetEnvironmentVariable('CC', $Cc, 'Process')
}
if ($Cflags) {
    [System.Environment]::SetEnvironmentVariable('CFLAGS', $Cflags, 'Process')
}
if ($PeersYml) {
    [System.Environment]::SetEnvironmentVariable('PEERS_YML', $PeersYml, 'Process')
}

$rustFlags = @(
    '--cfg', 'context="st-stm32f427vg"',
    '--cfg', 'context="stm32f427vg"',
    '--cfg', 'context="stm32"',
    '--cfg', 'context="ariel-os"',
    '--cfg', 'context="default"',
    '--cfg', 'capability="async-flash-driver"',
    '--cfg', 'capability="hw/stm32-usb-synopsys"',
    '--cfg', 'capability="hw/stm32-rng"',
    '--cfg', 'use_stm32_single_bank',
    '--cfg', 'stable',
    '-Cembed-bitcode=yes',
    '-Clto=fat',
    '-Ccodegen-units=1',
    '--cfg', 'capability="hw/device-identity"',
    '-Clink-arg=-Tdefmt.x',
    '--cfg', 'armv7m',
    '--cfg', 'armv7m_eabihf',
    '-Clink-arg=--nmagic',
    '-Clink-arg=--no-eh-frame-hdr',
    '-Clink-arg=-Tlinkme.x',
    '-Clink-arg=-Tlink.x',
    '-Clink-arg=-Teheap.x',
    '-Clink-arg=-Tdevice.x',
    '-Clink-arg=-Tisr_stack.x',
    '--cfg', 'context="cortex-m"',
    '--cfg', 'context="cortex-m4f"'
) -join ' '

[System.Environment]::SetEnvironmentVariable("${targetEnvPrefix}_RUNNER", 'probe-rs run --protocol=swd --chip STM32F427VGTx --preverify', 'Process')
[System.Environment]::SetEnvironmentVariable("${targetEnvPrefix}_RUSTFLAGS", $rustFlags, 'Process')

$cargoArgs = @(
    'build'
    "--manifest-path=$manifestPath"
    "--profile=$Profile"
)

if ($Features) {
    $cargoArgs += $Features
}

& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}