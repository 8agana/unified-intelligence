#!/usr/bin/env python3
"""Simple test for duplicate thought error handling."""

import json
import subprocess
import time

def send_request(method, params=None):
    """Send a JSON-RPC request to the MCP server."""
    request = {
        "jsonrpc": "2.0",
        "id": int(time.time() * 1000),
        "method": method,
        "params": params or {}
    }
    
    # Start the server process
    proc = subprocess.Popen(
        ["./target/release/unified-intelligence"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={
            "RUST_LOG": "info",
            "INSTANCE_ID": "TEST",
            "REDIS_PASSWORD": "8979323846"
        }
    )
    
    # Send request
    proc.stdin.write(json.dumps(request).encode() + b'\n')
    proc.stdin.flush()
    
    # Read response
    response_line = proc.stdout.readline()
    response = json.loads(response_line.decode())
    
    # Terminate process
    proc.terminate()
    proc.wait()
    
    return response


def main():
    print("Testing duplicate thought detection...")
    
    # Initialize
    print("\n1. Initializing...")
    response = send_request("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {}
    })
    print(f"Initialize response: {json.dumps(response, indent=2)}")
    
    # First thought
    print("\n2. Creating first thought...")
    response = send_request("tools/call", {
        "name": "ui_think",
        "arguments": {
            "thought": "Test thought for duplicate detection",
            "thought_number": 1,
            "total_thoughts": 2,
            "next_thought_needed": True
        }
    })
    print(f"First thought response: {json.dumps(response, indent=2)}")
    
    # Duplicate thought
    print("\n3. Attempting duplicate thought...")
    response = send_request("tools/call", {
        "name": "ui_think",
        "arguments": {
            "thought": "Test thought for duplicate detection",
            "thought_number": 2,
            "total_thoughts": 2,
            "next_thought_needed": False
        }
    })
    print(f"Duplicate thought response: {json.dumps(response, indent=2)}")
    
    # Check for error
    if "error" in response:
        print(f"\n✓ SUCCESS: Duplicate correctly rejected with error: {response['error']['message']}")
    else:
        print("\n✗ FAILURE: Duplicate was not rejected!")


if __name__ == "__main__":
    main()