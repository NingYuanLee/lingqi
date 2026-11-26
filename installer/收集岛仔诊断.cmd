@echo off
rem Portable package: the diagnostics script is embedded in lingqi.exe.
start "" "%~dp0lingqi.exe" --collect-diagnostics
