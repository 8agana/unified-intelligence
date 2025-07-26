#!/bin/bash

# Export required environment variables
export REDIS_PASSWORD=legacymind_redis_pass
export INSTANCE_ID=DT

# Create the MCP requests (first think, then recall)
cat > /tmp/mcp_requests.jsonl << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ui_think","arguments":{"thought":"This is a test thought stored by the refactored cognitive engine","thought_number":1,"total_thoughts":1,"next_thought_needed":false}}}
EOF

# Run the MCP server with the requests
echo "=== STORING THOUGHT ==="
(cat /tmp/mcp_requests.jsonl; sleep 1) | ./target/release/unified-intelligence 2>/tmp/mcp_stderr.log | tee /tmp/mcp_stdout.log

# Extract the thought ID from the response
THOUGHT_ID=$(cat /tmp/mcp_stdout.log | grep -A 100 '"id":2' | grep '"result"' | jq -r '.result.content[0].text' | jq -r '.thought_id')

echo -e "\n=== THOUGHT STORED WITH ID: $THOUGHT_ID ==="

# Now recall the thought
cat > /tmp/mcp_recall.jsonl << EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ui_recall","arguments":{"mode":"Thought","id":"$THOUGHT_ID"}}}
EOF

echo -e "\n=== RECALLING THOUGHT ==="
(cat /tmp/mcp_recall.jsonl; sleep 1) | ./target/release/unified-intelligence 2>/tmp/mcp_stderr2.log | tee /tmp/mcp_recall_stdout.log

# Extract the recall result
echo -e "\n=== RECALL RESULT ==="
cat /tmp/mcp_recall_stdout.log | grep -A 100 '"id":2' | grep '"result"' | jq -r '.result.content[0].text' | jq .