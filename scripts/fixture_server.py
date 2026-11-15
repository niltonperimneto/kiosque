"""Synthetic API for desktop tests. Only listens on loopback; never proxies requests."""
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import threading
import time

FIXTURES = Path(__file__).resolve().parents[1] / 'tests/fixtures/http'


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        data = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))) or '{}')
        query = data.get('query', '')
        if query == 'slow':
            time.sleep(0.3)
        if query == 'error':
            return self.respond(503, {'error': 'fixture unavailable'})
        self.respond(200, {'hits': [] if query == 'empty' else [{'app_id': f'org.kiosque.{query or "Fixture"}', 'name': query or 'Fixture'}]})

    def do_GET(self):
        if self.path.startswith('/collection/'):
            return self.respond(200, json.loads((FIXTURES / 'collection.json').read_text()))
        if self.path.startswith('/appstream/'):
            return self.respond(200, json.loads((FIXTURES / 'details.json').read_text()))
        if self.path.startswith('/ratings/'):
            return self.respond(200, json.loads((FIXTURES / 'ratings.json').read_text()))
        if self.path.startswith('/app/'):
            return self.respond(200, [])
        self.respond(200, {})

    def respond(self, status, value):
        data = json.dumps(value).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass


@contextmanager
def fixture_server():
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f'http://127.0.0.1:{server.server_port}'
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
