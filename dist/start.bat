@echo off
rem Emulateur TRS-80 Model I : lance le serveur local et ouvre le fureteur.
rem TRS-80 Model I emulator: starts the local server and opens the browser.
cd /d "%~dp0"
set PY=python
where python >nul 2>nul || set PY=py
where %PY% >nul 2>nul || (
  echo Python 3 est requis / Python 3 is required: https://www.python.org/downloads/
  pause
  exit /b 1
)
rem Ouvre la page apres deux secondes, le temps que le serveur demarre.
start "" /b cmd /c "timeout /t 2 /nobreak >nul & start http://localhost:8080"
%PY% serve.py 8080
pause
