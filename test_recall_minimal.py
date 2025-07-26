#!/usr/bin/env python3
"""
Minimal test for ui_recall
"""

import json
import sys
import os

# Test messages that will be sent to the MCP server
messages = [
    # First: Initialize
    {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0",
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            },
            "capabilities": {}
        }
    },
    # Second: Call ui_recall
    {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "ui_recall",
            "arguments": {
                "mode": "Thought",
                "id": "1381370a-e16b-42f4-a784-9deee79e1e27"
            }
        }
    }
]

# Print each message (this will be piped to the MCP server)
for msg in messages:
    print(json.dumps(msg))
    sys.stdout.flush()