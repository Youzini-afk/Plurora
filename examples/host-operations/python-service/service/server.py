import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/healthz":
            body = b"ok\n"
            content_type = "text/plain; charset=utf-8"
        else:
            body = json.dumps(
                {"service": "plurora-python-fixture", "path": self.path},
                separators=(",", ":"),
            ).encode()
            content_type = "application/json"

        self.send_response(200)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format, *_args):
        return


ThreadingHTTPServer(("0.0.0.0", 8000), Handler).serve_forever()
