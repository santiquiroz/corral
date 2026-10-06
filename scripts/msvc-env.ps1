$installer = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer"
$env:PATH = "$installer;$env:PATH"
$installs = & "$installer\vswhere.exe" -all -products * -property installationPath
# Algunos installs traen cl.exe sin headers del toolset (C1083 'excpt.h'); se elige uno completo
$vs = $installs | Where-Object { Get-ChildItem "$_\VC\Tools\MSVC\*\include\excpt.h" -ErrorAction SilentlyContinue } | Select-Object -First 1
if (-not $vs) { throw "No hay un Visual Studio con toolset MSVC completo (falta excpt.h)" }
Import-Module "$vs\Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments "-arch=x64 -host_arch=x64" | Out-Null
Write-Host "MSVC listo: $vs (VCToolsVersion=$env:VCToolsVersion)"
