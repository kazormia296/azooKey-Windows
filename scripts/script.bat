@REM net use Z: "\\vmware-host\Shared Folders" /persistent:yes
regsvr32.exe "C:\Users\user\Documents\azooKey-Windows\target\debug\tsf_core.dll" /u
@REM regsvr32.exe "D:\azookey-windows\build\x86\tsf_core.dll" /u /s
@REM start D:\azookey-windows\build\launcher.exe
regsvr32.exe "C:\Users\user\Documents\azooKey-Windows\target\debug\tsf_core.dll"
@REM regsvr32.exe "D:\azookey-windows\build\x86\tsf_core.dll" /s