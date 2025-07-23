#!/usr/bin/env python3
"""Test script to verify duplicate thought error handling."""

import asyncio
import json
import sys
from contextlib import asynccontextmanager
from typing import Any, Dict, Optional

import rmcp
from rmcp.client import ClientSession


@asynccontextmanager
async def create_client():
    """Create an MCP client for testing."""
    try:
        async with ClientSession() as session:
            # Launch the server
            client = await session.initialize_with_servers(
                {
                    "unified-intelligence": {
                        "command": "./target/release/unified-intelligence",
                        "env": {
                            "RUST_LOG": "info",
                            "INSTANCE_ID": "TEST",
                            "REDIS_PASSWORD": "8979323846"
                        }
                    }
                }
            )
            
            # Verify tools are available
            tools = await client.list_tools()
            tool_names = [tool.name for tool in tools]
            print(f"Available tools: {tool_names}")
            
            if "ui_think" not in tool_names:
                raise Exception("ui_think tool not found!")
            
            yield client
    except Exception as e:
        print(f"Failed to initialize client: {e}")
        sys.exit(1)


async def test_duplicate_thought():
    """Test duplicate thought error handling."""
    async with create_client() as client:
        thought_content = "This is a test thought for duplicate detection"
        
        # First thought should succeed
        print("\n1. Creating first thought...")
        result1 = await client.call_tool(
            "unified-intelligence",
            "ui_think",
            {
                "thought": thought_content,
                "thought_number": 1,
                "total_thoughts": 2,
                "next_thought_needed": True,
                "tags": ["test", "duplicate"]
            }
        )
        
        if result1 and result1.content:
            print(f"✓ First thought created successfully: {result1.content[0].text}")
        else:
            print("✗ Failed to create first thought")
            return
        
        # Second identical thought should fail
        print("\n2. Attempting to create duplicate thought...")
        try:
            result2 = await client.call_tool(
                "unified-intelligence",
                "ui_think",
                {
                    "thought": thought_content,
                    "thought_number": 2,
                    "total_thoughts": 2,
                    "next_thought_needed": False,
                    "tags": ["test", "duplicate"]
                }
            )
            
            # If we get here without error, something's wrong
            print("✗ Duplicate thought was allowed! This should not happen.")
            if result2 and result2.content:
                print(f"Result: {result2.content[0].text}")
        
        except Exception as e:
            print(f"✓ Duplicate thought correctly rejected with error: {e}")
            
            # Check if it's the expected error message
            error_str = str(e)
            if "Duplicate thought detected" in error_str:
                print("✓ Error message contains expected duplicate detection message")
            else:
                print(f"⚠ Unexpected error message: {error_str}")
        
        # Third thought with different content should succeed
        print("\n3. Creating third thought with different content...")
        result3 = await client.call_tool(
            "unified-intelligence",
            "ui_think",
            {
                "thought": "This is a different thought that should succeed",
                "thought_number": 2,
                "total_thoughts": 2,
                "next_thought_needed": False,
                "tags": ["test", "different"]
            }
        )
        
        if result3 and result3.content:
            print(f"✓ Different thought created successfully: {result3.content[0].text}")
        else:
            print("✗ Failed to create different thought")


async def main():
    """Run the test."""
    print("Testing UnifiedIntelligence duplicate thought error handling...")
    print("=" * 60)
    
    try:
        await test_duplicate_thought()
        print("\n" + "=" * 60)
        print("Test completed!")
    except Exception as e:
        print(f"\nTest failed with error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())