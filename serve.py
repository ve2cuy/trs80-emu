"""Serveur de développement : sert le dossier www/ sans mise en cache.

    python serve.py            # http://localhost:8080
    python serve.py 9000       # autre port

Contrairement à « python -m http.server », chaque réponse interdit la mise en cache :
après une modification (ou un nouveau wasm-pack build), un simple rechargement de la
page suffit. C'est aussi ce serveur qui rend visibles les disquettes locales
(www/disks/local/), absentes du site publié.
"""
import functools
import http.server
import os
import sys


class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    root = os.path.dirname(os.path.abspath(__file__))
    handler = functools.partial(NoCacheHandler, directory=root)
    with http.server.ThreadingHTTPServer(('127.0.0.1', port), handler) as server:
        print(f'TRS-80 : http://localhost:{port}  (Ctrl+C pour arrêter)')
        server.serve_forever()


if __name__ == '__main__':
    main()
