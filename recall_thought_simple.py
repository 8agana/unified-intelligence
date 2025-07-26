#!/usr/bin/env python3
"""
Simple script to recall a specific thought using ui_recall
"""

import json
import subprocess
import os
import time

def send_mcp_requests():
    # Set environment variables
    env = os.environ.copy()
    env['REDIS_PASSWORD'] = 'legacymind_redis_pass'
    env['INSTANCE_ID'] = 'DT'
    
    # Create requests
    requests = [
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "recall-client",
                    "version": "1.0.0"
                }
            }
        },
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
    
    # Write requests to a temp file
    with open('/tmp/mcp_requests.jsonl', 'w') as f:
        for req in requests:
            f.write(json.dumps(req) + '\n')
    
    # Run the command
    result = subprocess.run(
        ['./target/release/unified-intelligence'],
        stdin=open('/tmp/mcp_requests.jsonl', 'r'),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env
    )
    
    # Parse output
    for line in result.stdout.strip().split('\n'):
        if line:
            try:
                response = json.loads(line)
                if response.get("id") == 2:  # Our recall request
                    if "result" in response:
                        content = response["result"]["content"][0]["text"]
                        thought_data = json.loads(content)
                        print(json.dumps(thought_data, indent=2))
                    else:
                        print(f"Error: {response}")
            except json.JSONDecodeError:
                pass
    
    if result.returncode != 0:
        print(f"Stderr: {result.stderr}")

if __name__ == "__main__":
    send_mcp_requests()