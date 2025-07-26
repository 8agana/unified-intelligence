#!/usr/bin/env python3
"""
Interactive script to recall a specific thought using ui_recall
"""

import json
import subprocess
import sys
import time
import os

class MCPClient:
    def __init__(self):
        self.process = None
        self.request_id = 0
    
    def start_server(self):
        """Start the MCP server"""
        print("Starting UnifiedIntelligence MCP Server...")
        env = os.environ.copy()
        env['REDIS_PASSWORD'] = 'legacymind_redis_pass'
        env['INSTANCE_ID'] = 'DT'
        
        self.process = subprocess.Popen(
            ["./target/release/unified-intelligence"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0,
            env=env,
            cwd="/Users/samuelatagana/Projects/LegacyMind/unified-intelligence/refactor-cognitive-engine"
        )
        time.sleep(3)  # Give server time to start
        print("Server started")
        
        # Check for startup errors
        if self.process.poll() is not None:
            stderr = self.process.stderr.read()
            print(f"Server failed to start. Stderr: {stderr}")
            sys.exit(1)
    
    def send_request(self, method: str, params: dict = None) -> dict:
        """Send a JSON-RPC request and get response"""
        self.request_id += 1
        request = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params or {}
        }
        
        print(f"\n→ Sending: {method}")
        
        # Send request
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        
        # Read response
        response_line = self.process.stdout.readline()
        if response_line:
            response = json.loads(response_line.strip())
            return response
        else:
            print("← No response received")
            return None
    
    def cleanup(self):
        """Clean up the server process"""
        if self.process:
            # Read any remaining stderr
            stderr = self.process.stderr.read()
            if stderr:
                print(f"\nServer stderr: {stderr}")
            
            self.process.terminate()
            self.process.wait()
            print("\nServer stopped")

def recall_thought():
    """Recall the specific thought"""
    client = MCPClient()
    
    try:
        # Start server
        client.start_server()
        
        # Initialize
        print("\n[1] Initialize")
        response = client.send_request("initialize", {
            "clientInfo": {
                "name": "recall-client",
                "version": "1.0.0"
            },
            "capabilities": {}
        })
        
        if not response or "result" not in response:
            print(f"Initialize failed: {response}")
            return
        
        print("✓ Initialize successful")
        
        # Recall thought
        print("\n[2] Recall Thought")
        response = client.send_request("tools/call", {
            "name": "ui_recall",
            "arguments": {
                "mode": "Thought",
                "id": "1381370a-e16b-42f4-a784-9deee79e1e27"
            }
        })
        
        if response and "result" in response:
            result = json.loads(response["result"]["content"][0]["text"])
            print("\n✓ Thought recalled successfully:")
            print(json.dumps(result, indent=2))
        else:
            print(f"❌ Recall failed: {response}")
        
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        client.cleanup()

if __name__ == "__main__":
    recall_thought()