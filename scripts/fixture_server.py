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
        if self.path == '/submit':
            if data.get('summary') == 'Reject fixture review':
                return self.respond(503, {'error': 'fixture rejected review'})
            with self.server.state_lock:
                reviews = self.server.reviews.setdefault(data['app_id'], [])
                review = dict(data, review_id=self.server.next_review_id, karma_up=0, karma_down=0)
                self.server.next_review_id += 1
                reviews.append(review)
            return self.respond(200, {'success': True})
        if self.path in ('/upvote', '/downvote', '/dismiss', '/remove'):
            with self.server.state_lock:
                for reviews in self.server.reviews.values():
                    for review in list(reviews):
                        if review['review_id'] != data['review_id']:
                            continue
                        if self.path in ('/remove', '/dismiss'):
                            reviews.remove(review)
                        else:
                            key = 'karma_up' if self.path == '/upvote' else 'karma_down'
                            review[key] = review.get(key, 0) + 1
            return self.respond(200, {'success': True})
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
            app_id = self.path.rsplit('/', 1)[1]
            with self.server.state_lock:
                first_failure = app_id == 'org.kiosque.Broken' and app_id not in self.server.failed_details
                self.server.failed_details.add(app_id)
            if first_failure:
                return self.respond(503, {'error': 'fixture detail failure'})
            if app_id == 'org.kiosque.Slow':
                time.sleep(0.3)
            details = json.loads((FIXTURES / 'details.json').read_text())
            details.update(id=app_id, name=app_id.rsplit('.', 1)[1])
            return self.respond(200, details)
        if self.path.startswith('/ratings/'):
            app_id = self.path.rsplit('/', 1)[1]
            with self.server.state_lock:
                if app_id in self.server.reviews:
                    reviews = self.server.reviews[app_id]
                    ratings = {'total': len(reviews)}
                    for star in range(6):
                        ratings[f'star{star}'] = sum(round(r['rating'] / 20) == star for r in reviews)
                    return self.respond(200, ratings)
            return self.respond(200, json.loads((FIXTURES / 'ratings.json').read_text()))
        if self.path.startswith('/app/'):
            app_id = self.path.rsplit('/', 1)[1]
            with self.server.state_lock:
                reviews = json.loads(json.dumps(self.server.reviews.get(app_id, [])))
            return self.respond(200, reviews)
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
    server.state_lock = threading.Lock()
    server.reviews = {'org.kiosque.Reviews': [{
        'app_id': 'org.kiosque.Reviews', 'locale': 'en_US', 'review_id': 10,
        'rating': 100, 'summary': 'Fixture review', 'description': 'Synthetic review',
        'user_display': 'Fixture reviewer', 'karma_up': 2, 'karma_down': 0,
    }]}
    server.next_review_id = 11
    server.failed_details = set()
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f'http://127.0.0.1:{server.server_port}'
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
