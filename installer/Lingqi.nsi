; 灵栖 Lingqi 的图形安装包（NSIS 3）。
;
; 与 installer/install.ps1 的便携安装保持一致：
;   - 装到用户目录 %LOCALAPPDATA%\Programs\Lingqi，不需要管理员权限；
;   - 桌面与开始菜单快捷方式（另有「灵栖设置」打开设置页）；
;   - 写入 HKCU 的卸载登记项，Windows「应用」里能正常卸载。
;
; 编译（tools/package-nsis.ps1 负责调用）：
;   makensis /DVERSION=0.1.4 /DSTAGE="...\Lingqi-v0.1.4-windows-x64" ^
;            /DICON="...\assets\lingqi.ico" /DOUTFILE="...\Lingqi-Setup-v0.1.4.exe" ^
;            installer\Lingqi.nsi

Unicode true
SetCompressor /SOLID lzma
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef STAGE
  !define STAGE "..\dist\Lingqi-v0.1.4-windows-x64"
!endif
!ifndef ICON
  !define ICON "..\assets\lingqi.ico"
!endif
!ifndef OUTFILE
  !define OUTFILE "Lingqi-Setup-v${VERSION}.exe"
!endif

Name "灵栖 Lingqi ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Lingqi"
InstallDirRegKey HKCU "Software\Lingqi" "InstallDir"
RequestExecutionLevel user
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "灵栖 Lingqi"
VIAddVersionKey "FileDescription" "灵栖 Lingqi —— Windows 顶部灵动岛桌宠"
VIAddVersionKey "FileVersion" "${VERSION}.0"
VIAddVersionKey "ProductVersion" "${VERSION}.0"
VIAddVersionKey "CompanyName" "灵栖"
VIAddVersionKey "LegalCopyright" "MIT"

!define MUI_ABORTWARNING
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\lingqi.exe"
!define MUI_FINISHPAGE_RUN_TEXT "启动灵栖"
!define MUI_FINISHPAGE_RUN_NOTCHECKED

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; 语言必须放在页面宏之后：MUI_INSERT 会补上默认图标与文案。
!insertmacro MUI_LANGUAGE "SimpChinese"

!define SHORTCUT_LINGQI "灵栖.lnk"
!define SHORTCUT_SETTINGS "灵栖设置.lnk"

Section "灵栖 Lingqi（推荐）" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${STAGE}\lingqi.exe"
  File "${STAGE}\LICENSE"
  File "${STAGE}\THIRD_PARTY_NOTICES.txt"
  File "${STAGE}\README.md"
  File "${STAGE}\install.ps1"
  File "${STAGE}\安装灵栖.cmd"
  File "${STAGE}\收集灵栖诊断.cmd"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; 卸载登记项
  WriteRegStr HKCU "Software\Lingqi" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "DisplayName" "灵栖 Lingqi"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "Publisher" "灵栖"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "DisplayIcon" "$INSTDIR\lingqi.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "NoRepair" 1
SectionEnd

Section "开始菜单与桌面快捷方式" SecShortcuts
  CreateDirectory "$SMPROGRAMS"
  CreateShortCut "$DESKTOP\${SHORTCUT_LINGQI}" "$INSTDIR\lingqi.exe"
  CreateShortCut "$SMPROGRAMS\${SHORTCUT_LINGQI}" "$INSTDIR\lingqi.exe"
  CreateShortCut "$SMPROGRAMS\${SHORTCUT_SETTINGS}" "$INSTDIR\lingqi.exe" "--settings"
SectionEnd

Section "卸载登记项（可在「应用」里卸载）" SecRegistry
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "DisplayName" "灵栖 Lingqi"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi" "NoRepair" 1
SectionEnd

LangString DESC_SecMain ${LANG_SIMPCHINESE} "程序本体、许可证与说明文件。"
LangString DESC_SecShortcuts ${LANG_SIMPCHINESE} "在桌面与开始菜单放置灵栖与「灵栖设置」的入口。"
LangString DESC_SecRegistry ${LANG_SIMPCHINESE} "在 Windows「应用」里登记卸载入口。"

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "$(DESC_SecMain)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecShortcuts} "$(DESC_SecShortcuts)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecRegistry} "$(DESC_SecRegistry)"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$DESKTOP\${SHORTCUT_LINGQI}"
  Delete "$SMPROGRAMS\${SHORTCUT_LINGQI}"
  Delete "$SMPROGRAMS\${SHORTCUT_SETTINGS}"
  Delete "$INSTDIR\lingqi.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\install.ps1"
  Delete "$INSTDIR\安装灵栖.cmd"
  Delete "$INSTDIR\收集灵栖诊断.cmd"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lingqi"
  DeleteRegKey HKCU "Software\Lingqi"
SectionEnd
