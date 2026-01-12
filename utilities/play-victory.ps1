Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class WinMM {
    [DllImport("winmm.dll")]
    public static extern int mciSendString(string command, System.Text.StringBuilder buffer, int bufferSize, IntPtr hwndCallback);
}
"@
[WinMM]::mciSendString("open `"C:\dev\claude-playground-1\utilities\victory-sting.mp3`" type mpegvideo alias victory", $null, 0, [IntPtr]::Zero)
[WinMM]::mciSendString("play victory wait", $null, 0, [IntPtr]::Zero)
[WinMM]::mciSendString("close victory", $null, 0, [IntPtr]::Zero)
