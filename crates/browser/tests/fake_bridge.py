#!/usr/bin/env python3
"""A loopback HTTP stand-in for the Android WebView bridge.

Answers every POST with the JSON the Rust client expects, so the transport
can be tested without a phone.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length)
        try:
            request = json.loads(raw)
        except json.JSONDecodeError:
            request = {}

        method = request.get("method")
        params = request.get("params") or {}

        if method == "navigate":
            output = "loaded " + str(params.get("url", ""))
        elif method == "evaluate":
            output = "42"
        elif method == "screenshot":
            output = "iVBORw0KGgo="
        else:
            output = "unknown"

        body = json.dumps({
            "id": request.get("id"),
            "ok": True,
            "output": output,
        }).encode()

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8099
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
