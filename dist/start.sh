#!/bin/sh
# Émulateur TRS-80 Model I : lance le serveur local et ouvre le fureteur.
# TRS-80 Model I emulator: starts the local server and opens the browser.
cd "$(dirname "$0")" || exit 1
PY=$(command -v python3 || command -v python)
if [ -z "$PY" ]; then
  echo "Python 3 est requis / Python 3 is required: https://www.python.org/downloads/"
  exit 1
fi
# Ouvre la page après deux secondes, le temps que le serveur démarre.
( sleep 2; (xdg-open http://localhost:8080 || open http://localhost:8080) >/dev/null 2>&1 ) &
exec "$PY" serve.py 8080
