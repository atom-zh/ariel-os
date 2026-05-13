param(
    [string]$BoardsDir = "boards",
    [string]$OutputDir = "src/ariel-os-boards",
    [ValidateSet("update", "overwrite")]
    [string]$Mode = "update"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-RepoPath([string]$Path) {
    return [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\$Path"))
}

function Remove-LocalExtensionBlocks([string]$Content) {
    $result = New-Object System.Collections.Generic.List[string]
    $lines = $Content -split "`r?`n"
    $skipping = $false

    foreach ($line in $lines) {
        if (-not $skipping -and $line -match '^\s{4}(modem|tbox_log):\s*$') {
            $skipping = $true
            continue
        }

        if ($skipping) {
            if ($line -match '^\s{4}\S') {
                $skipping = $false
            }
            else {
                continue
            }
        }

        $result.Add($line)
    }

    return ($result -join "`r`n") + "`r`n"
}

function Get-YamlScalar([string]$Content, [string]$Pattern, [string]$Default = "") {
    $match = [regex]::Match($Content, $Pattern, [System.Text.RegularExpressions.RegexOptions]::Multiline)
    if ($match.Success) {
        return $match.Groups[1].Value.Trim()
    }
    return $Default
}

function Get-LogSetting([string]$Content, [string]$Name, [string]$Field, [string]$Default) {
    $blockPattern = '(?ms)^\s{6}' + [regex]::Escape($Name) + ':\s*$.*?(?=^\s{6}\S|^\s{4}\S|\z)'
    $block = [regex]::Match($Content, $blockPattern)
    if (-not $block.Success) {
        return $Default
    }

    return Get-YamlScalar $block.Value ('^\s{8}' + [regex]::Escape($Field) + ':\s*(.+)$') $Default
}

function Patch-Stm32f427vgBoard([string]$BoardYamlPath, [string]$GeneratedRsPath) {
    $yaml = Get-Content -Raw $BoardYamlPath
    $rs = Get-Content -Raw $GeneratedRsPath

    $model = Get-YamlScalar $yaml '^\s{6}model:\s*(.+)$' 'unknown'
    $uart = Get-YamlScalar $yaml '^\s{6}uart:\s*(.+)$' 'USART3'
    $baudrate = Get-YamlScalar $yaml '^\s{6}baudrate:\s*(.+)$' '115200'
    $uartDma = (Get-YamlScalar $yaml '^\s{6}uart_dma:\s*(.+)$' 'false').ToLowerInvariant()
    $rxPin = Get-YamlScalar $yaml '^\s{6}rx_pin:\s*(.+)$' 'PD9'
    $txPin = Get-YamlScalar $yaml '^\s{6}tx_pin:\s*(.+)$' 'PD8'

    $powerBlock = [regex]::Match($yaml, '(?ms)^\s{6}power:\s*$.*?(?=^\s{6}\S|^\s{4}\S|\z)')
    if (-not $powerBlock.Success) {
        throw "Could not locate power block in $BoardYamlPath"
    }
    $powerText = $powerBlock.Value
    $powerPin = Get-YamlScalar $powerText '^\s{8}pin:\s*(.+)$' 'PD15'
    $powerActive = Get-YamlScalar $powerText '^\s{8}active:\s*(.+)$' 'low'

    $pwrkeyBlock = [regex]::Match($yaml, '(?ms)^\s{6}pwrkey:\s*$.*?(?=^\s{4}\S|\z)')
    if (-not $pwrkeyBlock.Success) {
        throw "Could not locate pwrkey block in $BoardYamlPath"
    }
    $pwrkeyText = $pwrkeyBlock.Value
    $pwrkeyPin = Get-YamlScalar $pwrkeyText '^\s{8}pin:\s*(.+)$' 'PD13'
    $pwrkeyActive = Get-YamlScalar $pwrkeyText '^\s{8}active:\s*(.+)$' 'low'
    $pwrkeyPulseMs = Get-YamlScalar $pwrkeyText '^\s{8}pulse_ms:\s*(.+)$' '1000'

    $tboxLogBlock = [regex]::Match($yaml, '(?ms)^\s{4}tbox_log:\s*$.*?(?=^\s{4}\S|\z)')
    $tboxLogText = if ($tboxLogBlock.Success) { $tboxLogBlock.Value } else { "" }
    $kernelLogTag = Get-LogSetting $tboxLogText 'kernel' 'tag' 'kernel'
    $kernelLogEnabled = (Get-LogSetting $tboxLogText 'kernel' 'enabled' 'true').ToLowerInvariant()
    $modemLogTag = Get-LogSetting $tboxLogText 'modem' 'tag' 'modem'
    $modemLogEnabled = (Get-LogSetting $tboxLogText 'modem' 'enabled' 'false').ToLowerInvariant()
    $canLogTag = Get-LogSetting $tboxLogText 'can' 'tag' 'can'
    $canLogEnabled = (Get-LogSetting $tboxLogText 'can' 'enabled' 'true').ToLowerInvariant()

    $rs = [regex]::Replace($rs, '(?ms)\n\s*ariel_os_hal::define_peripherals!\(\s*ModemPeripherals \{.*?\n\s*\);', '')
    $rs = [regex]::Replace($rs, '(?m)^pub type ModemUart<''a> = .*?;\r?\n?', '')
    $rs = [regex]::Replace($rs, '(?ms)\n?pub mod modem \{.*?\n\}\r?\n?', '')
    $rs = [regex]::Replace($rs, '(?ms)\n?pub mod tbox_log \{.*?\n\}\r?\n(?=\r?\n#\[allow\(unused_variables\)\])', '')

    $powerActiveHigh = [string]($powerActive -eq 'high')
    $pwrkeyActiveHigh = [string]($pwrkeyActive -eq 'high')

    $modemPeripherals = @"
    ariel_os_hal::define_peripherals!(
        ModemPeripherals {
            modem_power: $powerPin,
            modem_pwrkey: $pwrkeyPin,
            modem_rx: $rxPin,
            modem_tx: $txPin,
        }
    );
"@

    $injected = @"
$modemPeripherals
}

pub type ModemUart<'a> = ariel_os_hal::hal::uart::$uart<'a>;

pub mod modem {
    pub const MODEL: &str = "$model";
    pub const UART: &str = "$uart";
    pub const BAUDRATE: u32 = $baudrate;
    pub const UART_DMA: bool = $uartDma;
    pub const RX_PIN: &str = "$rxPin";
    pub const TX_PIN: &str = "$txPin";
    pub const POWER_PIN: &str = "$powerPin";
    pub const PWRKEY_PIN: &str = "$pwrkeyPin";
    pub const POWER_ACTIVE_HIGH: bool = $($powerActiveHigh.ToLowerInvariant());
    pub const PWRKEY_ACTIVE_HIGH: bool = $($pwrkeyActiveHigh.ToLowerInvariant());
    pub const PWRKEY_PULSE_MS: u64 = $pwrkeyPulseMs;
}

pub mod tbox_log {
    pub mod kernel {
        pub const TAG: &str = "$kernelLogTag";
        pub const ENABLED: bool = $kernelLogEnabled;
    }

    pub mod modem {
        pub const TAG: &str = "$modemLogTag";
        pub const ENABLED: bool = $modemLogEnabled;
    }

    pub mod can {
        pub const TAG: &str = "$canLogTag";
        pub const ENABLED: bool = $canLogEnabled;
    }
}

#[allow(unused_variables)]
"@

    $rs = [regex]::Replace($rs, '\}\r?\n\r?\n#\[allow\(unused_variables\)\]', [System.Text.RegularExpressions.MatchEvaluator]{ param($m) $injected }, 1)

    Set-Content -Path $GeneratedRsPath -Value $rs -Encoding UTF8NoBOM
}

$boardsDirPath = Get-RepoPath $BoardsDir
$outputDirPath = Get-RepoPath $OutputDir
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ariel-os-boardgen-" + [System.Guid]::NewGuid().ToString('N'))
$tempBoards = Join-Path $tempRoot "boards"

try {
    New-Item -ItemType Directory -Path $tempBoards -Force | Out-Null
    Copy-Item -Path (Join-Path $boardsDirPath '*') -Destination $tempBoards -Recurse -Force

    $stmBoard = Join-Path $tempBoards "st-stm32f427vg.yaml"
    if (Test-Path $stmBoard) {
        $sanitized = Remove-LocalExtensionBlocks (Get-Content -Raw $stmBoard)
        Set-Content -Path $stmBoard -Value $sanitized -Encoding UTF8NoBOM
    }

    & sbd-gen generate-ariel $tempBoards -o $outputDirPath --mode $Mode
    if ($LASTEXITCODE -ne 0) {
        throw "sbd-gen failed with exit code $LASTEXITCODE"
    }

    Patch-Stm32f427vgBoard `
        -BoardYamlPath (Join-Path $boardsDirPath "st-stm32f427vg.yaml") `
        -GeneratedRsPath (Join-Path $outputDirPath "src\st-stm32f427vg.rs")
}
finally {
    if (Test-Path $tempRoot) {
        Remove-Item -Path $tempRoot -Recurse -Force
    }
}
