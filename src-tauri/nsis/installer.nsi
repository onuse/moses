; Moses NSIS Installer Script
; Handles WinFsp dependency automatically

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"
!include "FileFunc.nsh"

; General configuration
Name "Moses - Universal Filesystem Bridge"
OutFile "moses-setup.exe"
InstallDir "$PROGRAMFILES64\Moses"
InstallDirRegKey HKLM "Software\Moses" "InstallPath"
RequestExecutionLevel admin
ShowInstDetails show

; Version info
!define PRODUCT_VERSION "0.1.0"
!define PRODUCT_PUBLISHER "Moses Team"
!define WINFSP_URL "https://github.com/winfsp/winfsp/releases/download/v2.0/winfsp-2.0.23075.msi"
!define WINFSP_VERSION "2.0.23075"

; MUI Configuration
!define MUI_ABORTWARNING
!define MUI_ICON "..\icons\icon.ico"
!define MUI_UNICON "..\icons\icon.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "..\installer\sidebar.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "..\installer\header.bmp"

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
Page custom WinFspPage WinFspPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages
!insertmacro MUI_LANGUAGE "English"

; Variables
Var WinFspCheckbox
Var InstallWinFsp
Var Dialog

; Sections
Section "Moses Core (Required)" SecCore
    SectionIn RO
    
    SetOutPath "$INSTDIR"
    
    ; Install Moses files
    File "..\..\target\release\moses.exe"
    File "..\..\target\release\moses-worker.exe"
    
    ; Create uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"
    
    ; Registry entries
    WriteRegStr HKLM "Software\Moses" "InstallPath" "$INSTDIR"
    WriteRegStr HKLM "Software\Moses" "Version" "${PRODUCT_VERSION}"
    
    ; Add to PATH
    ${EnvVarUpdate} $0 "PATH" "A" "HKLM" "$INSTDIR"
    
    ; Start Menu shortcuts
    CreateDirectory "$SMPROGRAMS\Moses"
    CreateShortcut "$SMPROGRAMS\Moses\Moses.lnk" "$INSTDIR\moses.exe"
    CreateShortcut "$SMPROGRAMS\Moses\Uninstall.lnk" "$INSTDIR\uninstall.exe"
    
    ; Add to Windows "Add/Remove Programs"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses" \
                     "DisplayName" "Moses - Universal Filesystem Bridge"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses" \
                     "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses" \
                     "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses" \
                     "Publisher" "${PRODUCT_PUBLISHER}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses" \
                     "DisplayVersion" "${PRODUCT_VERSION}"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses" \
                      "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses" \
                      "NoRepair" 1
SectionEnd

Section "Desktop Shortcut" SecDesktop
    CreateShortcut "$DESKTOP\Moses.lnk" "$INSTDIR\moses.exe"
SectionEnd

Section "-WinFsp Installation" SecWinFsp
    ${If} $InstallWinFsp == 1
        DetailPrint "Downloading WinFsp..."
        
        ; Download WinFsp
        NSISdl::download "${WINFSP_URL}" "$TEMP\winfsp-installer.msi"
        Pop $0
        
        ${If} $0 == "success"
            DetailPrint "Installing WinFsp..."
            ExecWait 'msiexec /i "$TEMP\winfsp-installer.msi" /qn' $0
            
            ${If} $0 == 0
                DetailPrint "WinFsp installed successfully"
            ${Else}
                DetailPrint "WinFsp installation failed (code: $0)"
                MessageBox MB_OK|MB_ICONEXCLAMATION "WinFsp installation failed. Moses will work for formatting but not mounting."
            ${EndIf}
            
            Delete "$TEMP\winfsp-installer.msi"
        ${Else}
            DetailPrint "Failed to download WinFsp"
            MessageBox MB_OK|MB_ICONEXCLAMATION "Failed to download WinFsp. You can install it manually from https://winfsp.dev"
        ${EndIf}
    ${EndIf}
SectionEnd

; Component descriptions
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
    !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "Core Moses files (required)"
    !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Create a desktop shortcut"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; Custom page for WinFsp
Function WinFspPage
    ; Check if WinFsp is already installed
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\WinFsp" ""
    ${If} $0 != ""
        ; WinFsp already installed, skip this page
        StrCpy $InstallWinFsp 0
        Abort
    ${EndIf}
    
    !insertmacro MUI_HEADER_TEXT "WinFsp Installation" "WinFsp enables filesystem mounting"
    
    nsDialogs::Create 1018
    Pop $Dialog
    
    ${NSD_CreateLabel} 0 0 100% 40u "WinFsp (Windows File System Proxy) is required to mount filesystems as drives.$\r$\n$\r$\nWithout WinFsp, you can still format drives but cannot mount them.$\r$\n$\r$\nWinFsp is free and open source."
    
    ${NSD_CreateCheckbox} 0 50u 100% 10u "Install WinFsp ${WINFSP_VERSION}"
    Pop $WinFspCheckbox
    ${NSD_SetState} $WinFspCheckbox ${BST_CHECKED}
    
    ${NSD_CreateLabel} 0 70u 100% 20u "Note: This will download ~10MB and requires an internet connection."
    
    nsDialogs::Show
FunctionEnd

Function WinFspPageLeave
    ${NSD_GetState} $WinFspCheckbox $InstallWinFsp
FunctionEnd

; Uninstaller
Section "Uninstall"
    ; Remove from PATH
    ${un.EnvVarUpdate} $0 "PATH" "R" "HKLM" "$INSTDIR"
    
    ; Delete files
    Delete "$INSTDIR\moses.exe"
    Delete "$INSTDIR\moses-worker.exe"
    Delete "$INSTDIR\uninstall.exe"
    
    ; Delete shortcuts
    Delete "$DESKTOP\Moses.lnk"
    Delete "$SMPROGRAMS\Moses\*.*"
    RMDir "$SMPROGRAMS\Moses"
    
    ; Delete install directory
    RMDir "$INSTDIR"
    
    ; Delete registry entries
    DeleteRegKey HKLM "Software\Moses"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Moses"
    
    MessageBox MB_YESNO "Do you want to uninstall WinFsp as well?" IDNO +2
    ExecWait 'msiexec /x {C47F4273-232C-4E92-A4C5-03E52E81F3A4} /qn'
SectionEnd