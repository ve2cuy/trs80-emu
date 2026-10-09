# Relais telnet du modem

Le fureteur ne peut pas ouvrir de connexion TCP : le modem virtuel de l'émulateur (port
RS-232, `www/modem.js`) ouvre un WebSocket vers ce relais, qui se connecte au BBS par telnet.

```
TRS-80 (LCOMM) ─RS-232─► modem (page) ─wss://ve2cuy.com/trs80/relay─► Apache ─► relay.py ─telnet─► BBS
```

- `relay.py` : Python 3.8 ou plus récent, **bibliothèque standard seulement**.
- Serveurs joignables : `bbs.electrodrome.net:23`, plus les BBS de `bbs.json` (la liste de la
  page, `www/bbs.json`, tirée de <https://www.telnetbbsguide.com/bbs/list/brief/>), copié à
  côté de `relay.py` ou désigné par `--bbs`.
- Pages permises : `ORIGINS` (GitHub Pages, ve2cuy.com, localhost:8080).
- Limites : 20 connexions en tout, 3 par adresse IP, 30 minutes sans trafic.
- La négociation telnet est faite par le relais : le TRS-80 ne reçoit que le texte.

## Installation sur le serveur (Ubuntu, Apache)

```bash
sudo mkdir -p /opt/trs80-relay
sudo cp relay.py ../../www/bbs.json /opt/trs80-relay/
sudo cp trs80-relay.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now trs80-relay
systemctl status trs80-relay            # « relais à l'écoute sur 127.0.0.1:8023; 850 BBS permis »

sudo a2enmod proxy proxy_wstunnel
# Ajouter le contenu de apache-relay.conf dans le <VirtualHost *:443> de ve2cuy.com
# (/etc/apache2/sites-enabled/ve2cuy.com-le-ssl.conf), puis :
sudo apache2ctl configtest && sudo systemctl reload apache2
```

Journal : `journalctl -u trs80-relay -f` (connexions, refus).

## Essai local

```bash
python3 relay.py --bbs ../../www/bbs.json      # écoute sur 127.0.0.1:8023
```

Dans la page (`http://localhost:8080`), section Modem, « Adresse du relais telnet » :
`ws://localhost:8023`.

## Permettre un autre BBS

Ajouter `["Nom du BBS", "nom.du.bbs", port]` à `www/bbs.json` (la page l'affiche alors dans sa
liste), copier le fichier dans `/opt/trs80-relay/`, puis `sudo systemctl restart trs80-relay`.
