#!/bin/bash

# Export required environment variables
export REDIS_PASSWORD=legacymind_redis_pass
export INSTANCE_ID=DT

# Create the MCP requests
cat > /tmp/mcp_requests.jsonl << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ui_recall","arguments":{"mode":"Thought","id":"b3a916a9-d886-4f33-a998-c8ca93c18aa6"}}}
EOF

# Run the MCP server with the requests
(cat /tmp/mcp_requests.jsonl; sleep 1) | ./target/release/unified-intelligence 2>/tmp/mcp_stderr.log | tee /tmp/mcp_stdout.log

# Extract the recall result
echo -e "\n\n=== RECALL RESULT ==="
cat /tmp/mcp_stdout.log | grep -A 100 '"id":2' | grep '"result"' | jq -r '.result.content[0].text' | jq .

# Show any errors
if [ -s /tmp/mcp_stderr.log ]; then
    echo -e "\n=== STDERR LOG ==="
    cat /tmp/mcp_stderr.log
fi