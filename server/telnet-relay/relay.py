#!/usr/bin/env python3
"""Relais WebSocket -> telnet pour le modem de l'émulateur TRS-80.

Le fureteur ne peut pas ouvrir de connexion TCP : la page ouvre un WebSocket vers ce relais
(ws://127.0.0.1:8023/?host=bbs.electrodrome.net&port=23, publié par Apache en wss://), qui
ouvre la connexion telnet et fait passer les octets dans les deux sens.

- Seuls les serveurs de ALLOWED sont joignables (pas de relais ouvert à tout Internet) :
  bbs.electrodrome.net, plus ceux de la liste --bbs (bbs.json de la page : [nom, hôte, port]).
- Seules les pages de ORIGINS peuvent s'en servir (en-tête Origin du fureteur).
- La négociation telnet (octets IAC) est faite ici : le TRS-80 ne reçoit que le texte. Le
  relais accepte ECHO et SUPPRESS-GO-AHEAD du serveur, refuse le reste.
- Limites : MAX_CLIENTS connexions en tout, MAX_PER_IP par adresse, IDLE secondes sans
  trafic.

Bibliothèque standard seulement (Python 3.8 ou plus récent) : rien à installer.

    python3 relay.py [--listen 127.0.0.1] [--port 8023] [--bbs bbs.json]
"""
import argparse
import asyncio
import base64
import hashlib
import json
import logging
import os
import struct
from urllib.parse import parse_qs, urlsplit

ALLOWED = {('bbs.electrodrome.net', 23)}
ORIGINS = {
    'https://ve2cuy.github.io',
    'https://ve2cuy.com',
    'http://localhost:8080',
    'http://127.0.0.1:8080',
}
MAX_CLIENTS = 20
MAX_PER_IP = 3
IDLE = 30 * 60
GUID = b'258EAFA5-E914-47DA-95CA-C5AB0DC85B11'

# Telnet.
IAC, DONT, DO, WONT, WILL, SB, SE = 255, 254, 253, 252, 251, 250, 240
ECHO, SGA = 1, 3

log = logging.getLogger('relay')
clients = {}  # adresse IP -> nombre de connexions


class Closed(Exception):
    pass


async def read_request(reader):
    """En-tête HTTP de la demande de WebSocket : (chemin, en-têtes en minuscules)."""
    raw = await asyncio.wait_for(reader.readuntil(b'\r\n\r\n'), 10)
    lines = raw.decode('latin-1').split('\r\n')
    method, path, _ = lines[0].split(' ', 2)
    if method != 'GET':
        raise Closed('méthode ' + method)
    headers = {}
    for line in lines[1:]:
        if ':' in line:
            k, v = line.split(':', 1)
            headers[k.strip().lower()] = v.strip()
    return path, headers


def http_error(writer, status):
    writer.write(f'HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n'.encode())


async def read_frame(reader):
    """Un message WebSocket complet (fragments réunis) : (opcode, données)."""
    message, opcode = b'', None
    while True:
        b0, b1 = await reader.readexactly(2)
        fin, op = b0 & 0x80, b0 & 0x0F
        length = b1 & 0x7F
        if length == 126:
            (length,) = struct.unpack('>H', await reader.readexactly(2))
        elif length == 127:
            (length,) = struct.unpack('>Q', await reader.readexactly(8))
        if length > 1 << 20:
            raise Closed('message trop long')
        mask = await reader.readexactly(4) if b1 & 0x80 else None
        data = await reader.readexactly(length)
        if mask:
            data = bytes(b ^ mask[i & 3] for i, b in enumerate(data))
        if op >= 8:  # contrôle : jamais fragmenté
            return op, data
        if op:
            opcode = op
        message += data
        if fin:
            return opcode, message


def frame(opcode, data=b''):
    n = len(data)
    head = bytes([0x80 | opcode])
    if n < 126:
        head += bytes([n])
    elif n < 1 << 16:
        head += bytes([126]) + struct.pack('>H', n)
    else:
        head += bytes([127]) + struct.pack('>Q', n)
    return head + data


class Telnet:
    """Retire la négociation telnet du flux reçu et prépare les réponses."""

    def __init__(self):
        self.state = 'data'
        self.verb = 0

    def feed(self, data):
        text, replies = bytearray(), bytearray()
        for b in data:
            if self.state == 'data':
                if b == IAC:
                    self.state = 'iac'
                else:
                    text.append(b)
            elif self.state == 'iac':
                if b == IAC:
                    text.append(IAC)
                    self.state = 'data'
                elif b in (DO, DONT, WILL, WONT):
                    self.verb, self.state = b, 'option'
                elif b == SB:
                    self.state = 'sb'
                else:
                    self.state = 'data'
            elif self.state == 'option':
                if self.verb == WILL:
                    replies += bytes([IAC, DO if b in (ECHO, SGA) else DONT, b])
                elif self.verb == DO:
                    replies += bytes([IAC, WILL if b == SGA else WONT, b])
                self.state = 'data'
            elif self.state == 'sb':
                if b == IAC:
                    self.state = 'sb-iac'
            elif self.state == 'sb-iac':
                self.state = 'data' if b == SE else 'sb'
        return bytes(text), bytes(replies)


async def handle(reader, writer):
    peer = writer.get_extra_info('peername')
    # Derrière Apache, l'adresse du client est dans X-Forwarded-For.
    ip = peer[0] if peer else '?'
    telnet_writer = None
    counted = False
    try:
        path, headers = await read_request(reader)
        ip = headers.get('x-forwarded-for', ip).split(',')[0].strip()
        origin = headers.get('origin', '')
        query = parse_qs(urlsplit(path).query)
        host = query.get('host', [''])[0].lower()
        port = int(query.get('port', ['23'])[0] or 23)
        if headers.get('upgrade', '').lower() != 'websocket' or 'sec-websocket-key' not in headers:
            http_error(writer, '400 Bad Request')
            return
        if origin not in ORIGINS:
            log.info('%s refusé : origine %r', ip, origin)
            http_error(writer, '403 Forbidden')
            return
        if (host, port) not in ALLOWED:
            log.info('%s refusé : %s:%d non permis', ip, host, port)
            http_error(writer, '403 Forbidden')
            return
        if sum(clients.values()) >= MAX_CLIENTS or clients.get(ip, 0) >= MAX_PER_IP:
            http_error(writer, '503 Service Unavailable')
            return
        clients[ip] = clients.get(ip, 0) + 1
        counted = True
        accept = base64.b64encode(hashlib.sha1(headers['sec-websocket-key'].encode() + GUID).digest()).decode()
        writer.write(('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                      f'Sec-WebSocket-Accept: {accept}\r\n\r\n').encode())
        await writer.drain()

        try:
            telnet_reader, telnet_writer = await asyncio.wait_for(asyncio.open_connection(host, port), 20)
        except (OSError, asyncio.TimeoutError) as e:
            log.info('%s : %s:%d injoignable (%s)', ip, host, port, e)
            writer.write(frame(8, struct.pack('>H', 4004) + b'unreachable'))
            await writer.drain()
            return
        log.info('%s connecté à %s:%d', ip, host, port)
        negotiation = Telnet()

        async def to_telnet():
            while True:
                opcode, data = await asyncio.wait_for(read_frame(reader), IDLE)
                if opcode == 8:
                    return
                if opcode == 9:
                    writer.write(frame(10, data))
                elif opcode in (1, 2):
                    telnet_writer.write(data.replace(bytes([IAC]), bytes([IAC, IAC])))
                    await telnet_writer.drain()

        async def to_browser():
            while True:
                data = await asyncio.wait_for(telnet_reader.read(4096), IDLE)
                if not data:
                    return
                text, replies = negotiation.feed(data)
                if replies:
                    telnet_writer.write(replies)
                if text:
                    writer.write(frame(2, text))
                    await writer.drain()

        tasks = [asyncio.ensure_future(to_telnet()), asyncio.ensure_future(to_browser())]
        done, pending = await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
        for t in pending:
            t.cancel()
        for t in done:
            if t.exception() and not isinstance(t.exception(), (asyncio.IncompleteReadError, asyncio.TimeoutError,
                                                                  ConnectionError, Closed)):
                log.warning('%s : %r', ip, t.exception())
        writer.write(frame(8, struct.pack('>H', 1000)))
        log.info('%s déconnecté de %s:%d', ip, host, port)
    except (asyncio.IncompleteReadError, asyncio.TimeoutError, asyncio.LimitOverrunError,
            ConnectionError, ValueError, Closed) as e:
        log.debug('%s : %r', ip, e)
    finally:
        if counted:
            clients[ip] -= 1
            if not clients[ip]:
                del clients[ip]
        for w in (telnet_writer, writer):
            if w:
                try:
                    w.close()
                except Exception:
                    pass


async def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--listen', default=os.environ.get('RELAY_LISTEN', '127.0.0.1'))
    parser.add_argument('--port', type=int, default=int(os.environ.get('RELAY_PORT', '8023')))
    parser.add_argument('--bbs', default=os.path.join(os.path.dirname(os.path.abspath(__file__)), 'bbs.json'),
                        help='liste des BBS permis (absente : bbs.electrodrome.net seulement)')
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format='%(asctime)s %(message)s')
    try:
        with open(args.bbs, encoding='utf-8') as f:
            ALLOWED.update((str(host).lower(), int(port)) for _, host, port in json.load(f)['bbs'])
    except FileNotFoundError:
        log.info('%s absent : seul bbs.electrodrome.net est permis', args.bbs)
    server = await asyncio.start_server(handle, args.listen, args.port)
    log.info('relais à l\'écoute sur %s:%d; %d BBS permis', args.listen, args.port, len(ALLOWED))
    async with server:
        await server.serve_forever()


if __name__ == '__main__':
    asyncio.run(main())
