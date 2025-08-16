# Remote MCP Setup Instructions

## Required Components

For the unified-intelligence remote MCP to work, THREE things must be running:

### 1. Redis (Docker)
```bash
# Check if running
docker ps | grep redis

# If not running, start it
cd /Users/samuelatagana/Projects/LegacyMind/Memory/docker
docker-compose up -d
```

### 2. Unified-Intelligence HTTP Server
**Should start automatically via launchd on boot**

Check status:
```bash
# Check if running
ps aux | grep unified-intelligence | grep -v grep

# Check launchd status
launchctl list | grep legacymind
```

If not running, start manually:
```bash
# Via launchd (persistent)
launchctl bootstrap gui/501 /Users/samuelatagana/Library/LaunchAgents/dev.legacymind.ui.plist

# Or manually for testing
cd /Users/samuelatagana/Projects/LegacyMind/unified-intelligence
UI_TRANSPORT=http UI_HTTP_BIND=127.0.0.1:8787 UI_HTTP_PATH=/mcp ./target/release/unified-intelligence
```

### 3. Cloudflare Tunnel
Should start automatically via launchd (Homebrew service). If it isn’t, start it and enable it.

```bash
## Using Homebrew services (preferred)
brew services start cloudflared            # starts and enables at login
brew services restart cloudflared          # if already installed

## Using launchd directly
# Load/enable the LaunchAgent and kickstart it
launchctl enable gui/501/homebrew.mxcl.cloudflared
launchctl bootstrap gui/501 /Users/samuelatagana/Library/LaunchAgents/homebrew.mxcl.cloudflared.plist
launchctl kickstart -k gui/501/homebrew.mxcl.cloudflared

## Ad‑hoc (fallback)
nohup cloudflared tunnel run > /Users/samuelatagana/Library/Logs/cloudflared.out.log 2> /Users/samuelatagana/Library/Logs/cloudflared.err.log &
```

## Verification

Test all components:
```bash
# Test Redis
docker exec legacymind-redis redis-cli -a legacymind_redis_pass ping

# Test unified-intelligence locally
curl http://127.0.0.1:8787/health

# Test Cloudflare tunnel (remote access)
curl https://mcp.samataganaphotography.com/health
```

## Configuration Files

- **unified-intelligence launchd**: `/Users/samuelatagana/Library/LaunchAgents/dev.legacymind.ui.plist`
- **cloudflared config**: `/Users/samuelatagana/.cloudflared/config.yml`
- **cloudflared launchd**: `/Users/samuelatagana/Library/LaunchAgents/homebrew.mxcl.cloudflared.plist`

## Logs

- **unified-intelligence**: `/Users/samuelatagana/Library/Logs/unified-intelligence.*.log`
- **cloudflared**: `/Users/samuelatagana/Library/Logs/cloudflared.*.log`

## Remote URL for Claude Desktop

```
https://mcp.samataganaphotography.com/mcp?access_token=<token>
```

## Current Issues
If you see `error code: 1033` from the remote URL, the Cloudflare tunnel is not connected.

Runbook:
- Check LaunchAgent is loaded: `launchctl list | grep homebrew.mxcl.cloudflared`
- Kick it: `launchctl kickstart -k gui/501/homebrew.mxcl.cloudflared`
- Or `brew services restart cloudflared`
- Verify config: `~/.cloudflared/config.yml` must map `mcp.samataganaphotography.com -> http://localhost:8787`
- Tail logs: `tail -n 100 ~/Library/Logs/cloudflared.err.log`

If the unified server is down:
- `./scripts/ui_mcp.sh status` / `./scripts/ui_mcp.sh restart`
- Health: `curl http://127.0.0.1:8787/health` should return `ok`
