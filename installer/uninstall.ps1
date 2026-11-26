$ErrorActionPreference='Stop'
$installDir=[System.IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\Lingqi'))
if([System.IO.Path]::GetFullPath($PSScriptRoot) -ne $installDir){throw '卸载脚本只能在灵栖安装目录中运行。'}
$exePath=Join-Path $installDir 'lingqi.exe'
Get-Process lingqi -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $exePath} | ForEach-Object {
    $null=$_.CloseMainWindow();if(-not $_.WaitForExit(3000)){Stop-Process -Id $_.Id}
}
$shell=New-Object -ComObject WScript.Shell
$links=@((Join-Path $shell.SpecialFolders.Item('Desktop') '灵栖.lnk'),(Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\灵栖.lnk'),(Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\灵栖设置.lnk'),(Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\收集灵栖诊断.lnk'))
foreach($path in $links){if((Test-Path -LiteralPath $path) -and $shell.CreateShortcut($path).TargetPath -eq $exePath){Remove-Item -LiteralPath $path}}
$runKey='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
Remove-ItemProperty -Path $runKey -Name Lingqi -ErrorAction SilentlyContinue
Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi' -ErrorAction SilentlyContinue
foreach($name in @('lingqi.exe','lingqi-previous.exe','lingqi-v0.2-backup.exe','THIRD_PARTY_NOTICES.txt','LICENSE','uninstall.ps1','收集灵栖诊断.cmd','scripts\collect-diagnostics.ps1')){
    $path=Join-Path $installDir $name;if(Test-Path -LiteralPath $path){Remove-Item -LiteralPath $path}
}
$tools=Join-Path $installDir 'scripts'
if((Test-Path -LiteralPath $tools) -and -not(Get-ChildItem -LiteralPath $tools -Force)){Remove-Item -LiteralPath $tools}
if(-not(Get-ChildItem -LiteralPath $installDir -Force)){Remove-Item -LiteralPath $installDir}
Write-Output '灵栖已卸载。待办、饮水记录和设置仍保存在本机用户数据目录中。'
