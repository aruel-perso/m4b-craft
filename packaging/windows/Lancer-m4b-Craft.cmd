@echo off
setlocal
cd /d "%~dp0"
if not exist "%~dp0ffmpeg.exe" goto damaged
if not exist "%~dp0ffprobe.exe" goto damaged
start "" "%~dp0m4b-Craft.exe"
exit /b 0
:damaged
echo Archive incomplete : extraire TOUS les fichiers du ZIP dans un dossier.
echo Incomplete archive: extract ALL ZIP files into one folder.
echo Aucune installation externe requise / No external installation required.
pause
exit /b 1
