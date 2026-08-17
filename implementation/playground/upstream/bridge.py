# Copyright 2026 Salesforce, Inc. All rights reserved.

"""Streamable-HTTP to plain-JSON bridge in front of the A2D mock MCP server.

The A2D mock answers every `tools/call` with `text/event-stream`, even for a
single non-streaming result, and rejects an `Accept` header that does not
include it. The semantic-contract policy buffers and rewrites JSON bodies and
deliberately passes streams through untouched, so this bridge collapses the
single-frame SSE response back into `application/json` for the demo.

It is a demo shim, not part of the policy. See the SSE section of the README.
"""

import json
import os
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

UPSTREAM = os.environ["MCP_UPSTREAM_URL"]
API_KEY = os.environ["MCP_API_KEY"]


def collapse_sse(raw: str) -> bytes:
    """Returns the payload of the first SSE `data:` frame, or the body as-is."""
    data_lines = []
    for line in raw.splitlines():
        if line.startswith("data:"):
            payload = line[5:]
            data_lines.append(payload[1:] if payload.startswith(" ") else payload)
        elif not line.strip() and data_lines:
            break
    return "\n".join(data_lines).encode() if data_lines else raw.encode()


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length", 0)))
        request = urllib.request.Request(
            UPSTREAM,
            data=body,
            headers={
                "authorization": f"Bearer {API_KEY}",
                "content-type": "application/json",
                "accept": "application/json, text/event-stream",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = collapse_sse(response.read().decode())
                status = response.status
        except Exception as exc:  # noqa: BLE001 - surfaced to the caller as JSON-RPC
            payload = json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": None,
                    "error": {"code": -32603, "message": f"upstream failure: {exc}"},
                }
            ).encode()
            status = 502

        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, fmt, *args):
        print(f"bridge: {fmt % args}", flush=True)


if __name__ == "__main__":
    print(f"bridge: forwarding to {UPSTREAM}", flush=True)
    ThreadingHTTPServer(("0.0.0.0", 8000), Handler).serve_forever()
