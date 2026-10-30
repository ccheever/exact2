# AMD's supported Reset Shader Cache operation, used by S2.8b and S2.9.
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class G6Window {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
}
'@
$process=Get-Process RadeonSoftware
$condition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$process.Id)
$root=[System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children,$condition)
if ($null -eq $root) { throw 'Open AMD Software at Gaming > Graphics before this series' }
function Find-Button([string]$name) {
    $nameCondition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$name)
    $matches=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,$nameCondition)
    foreach ($item in $matches) {
        if ($item.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button) { return $item }
    }
    return $null
}
function Invoke-Button($button) {
    if ($null -eq $button) { throw 'Required AMD button absent' }
    $pattern=$button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $pattern.Invoke()
}
[void][G6Window]::ShowWindow([IntPtr]$root.Current.NativeWindowHandle,9)
Invoke-Button (Find-Button 'Reset Shader Cache Perform Reset')
$deadline=[DateTime]::UtcNow.AddSeconds(10)
do {
    $confirm=Find-Button 'OK'
    if ($null -ne $confirm) { break }
    Start-Sleep -Milliseconds 50
} while ([DateTime]::UtcNow -lt $deadline)
if ($null -eq $confirm) { throw 'Shader-cache confirmation absent' }
Invoke-Button $confirm
while ($null -ne (Find-Button 'OK')) {
    if ([DateTime]::UtcNow -ge $deadline) { throw 'Shader-cache confirmation did not close' }
    Start-Sleep -Milliseconds 50
}
[void][G6Window]::ShowWindow([IntPtr]$root.Current.NativeWindowHandle,6)
Write-Output ('Reset Shader Cache confirmed; AMD Software minimized; '+[DateTime]::UtcNow.ToString('o'))
