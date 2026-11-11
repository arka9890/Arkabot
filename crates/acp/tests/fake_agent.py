#!/usr/bin/env python3
"""A minimal ACP agent used by the Rust integration test.

It answers `initialize` and `session/new`, and on `session/prompt` sends an
`agent_message_chunk` update before the stop reason.
"""

import json
import sys

SESSION_ID = "sess_test"


def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue

    method = msg.get("method")
    msg_id = msg.get("id")

    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {
                "protocolVersion": 1,
                "agentCapabilities": {"loadSession": False},
                "authMethods": [],
            },
        })
    elif method == "session/new":
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {"sessionId": SESSION_ID},
        })
    elif method == "session/prompt":
        # Stream the reply in two chunks.
        send({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": SESSION_ID,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": {"type": "text", "text": "Hello "},
                },
            },
        })
        send({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": SESSION_ID,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": {"type": "text", "text": "from fake agent"},
                },
            },
        })
        send({
            "jsonrpc": "2.0",
            "id": msg_id,
            "result": {"stopReason": "end_turn"},
        })
