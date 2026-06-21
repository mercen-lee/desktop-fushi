param(
    [string]$Target = "desktop",
    [string]$Arch = "",
    [string]$Variant = "",
    [string]$Abis = "",
    [switch]$Debug
)

$ErrorActionPreference = "Stop"
$argsList = @($Target)
if ($Arch -ne "") { $argsList += @("--arch", $Arch) }
if ($Variant -ne "") { $argsList += @("--variant", $Variant) }
if ($Abis -ne "") { $argsList += @("--abis", $Abis) }
if ($Debug) { $argsList += "--debug" }
python "$PSScriptRoot\build.py" @argsList
