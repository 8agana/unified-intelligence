#!/usr/bin/env python3
"""
Script to recall a specific thought using ui_recall
"""

import json
import subprocess
import os

def recall_thought(thought_id):
    # Set environment variables
    env = os.environ.copy()
    env['REDIS_PASSWORD'] = 'legacymind_redis_pass'
    env['INSTANCE_ID'] = 'DT'
    
    # Start the process
    process = subprocess.Popen(
        ["./target/release/unified-intelligence"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env
    )
    
    # Send initialize request
    init_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "recall-client",
                "version": "1.0.0"
            }
        }
    }
    
    process.stdin.write(json.dumps(init_request) + "\n")
    process.stdin.flush()
    
    # Read initialize response
    init_response = process.stdout.readline()
    print(f"Initialize response: {init_response}")
    
    # Check stderr for errors
    stderr_output = process.stderr.read()
    if stderr_output:
        print(f"Stderr: {stderr_output}")
        return
    
    # Send ui_recall request
    recall_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "ui_recall",
            "arguments": {
                "mode": "Thought",
                "id": thought_id
            }
        }
    }
    
    process.stdin.write(json.dumps(recall_request) + "\n")
    process.stdin.flush()
    
    # Read recall response
    recall_response = process.stdout.readline()
    if recall_response:
        response = json.loads(recall_response)
        if "result" in response:
            result = json.loads(response["result"]["content"][0]["text"])
            print(json.dumps(result, indent=2))
        else:
            print(f"Error: {response}")
    
    # Clean up
    process.terminate()
    process.wait()

if __name__ == "__main__":
    recall_thought("1381370a-e16b-42f4-a784-9deee79e1e27")