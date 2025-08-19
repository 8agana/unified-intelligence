#!/usr/bin/env python3
"""
Script to consolidate all Warp thoughts under a single chain_id for comprehensive summarization.
"""

import redis
import json
import os
import sys
from datetime import datetime

def main():
    # Configuration
    INSTANCE_ID = "Warp"
    NEW_CHAIN_ID = "initial-summary"
    
    # Redis connection parameters
    redis_host = os.getenv("REDIS_HOST", "localhost")
    redis_port = int(os.getenv("REDIS_PORT", 6379))
    redis_password = os.getenv("REDIS_AUTH", "")
    
    print(f"🔗 Connecting to Redis at {redis_host}:{redis_port}")
    
    try:
        # Connect to Redis
        if redis_password:
            r = redis.Redis(host=redis_host, port=redis_port, password=redis_password, decode_responses=True)
        else:
            r = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
        
        # Test connection
        r.ping()
        print("✅ Connected to Redis")
        
    except redis.ConnectionError as e:
        print(f"❌ Failed to connect to Redis: {e}")
        print("\nTrying without auth...")
        try:
            r = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
            r.ping()
            print("✅ Connected to Redis (no auth)")
        except:
            print("❌ Failed to connect without auth either")
            sys.exit(1)
    
    # Find all Warp thoughts
    print(f"\n🔍 Searching for {INSTANCE_ID}:thoughts:* keys...")
    pattern = f"{INSTANCE_ID}:thoughts:*"
    thought_keys = list(r.scan_iter(match=pattern))
    
    if not thought_keys:
        print(f"⚠️  No thoughts found with pattern: {pattern}")
        print("\n🔍 Trying alternative patterns...")
        
        # Try case variations ONLY for Warp
        alt_patterns = [
            "Warp:Thoughts:*",  # Capital T as user specified
            "warp:thoughts:*",  # Lowercase variant
            "WARP:thoughts:*"   # Uppercase variant
        ]
        
        for alt_pattern in alt_patterns:
            alt_keys = list(r.scan_iter(match=alt_pattern))
            if alt_keys:
                print(f"✅ Found {len(alt_keys)} thoughts with pattern: {alt_pattern}")
                thought_keys = alt_keys
                break
    
    if not thought_keys:
        print("❌ No thoughts found in Redis")
        sys.exit(1)
    
    print(f"📊 Found {len(thought_keys)} thought keys")
    
    # Track chain IDs before update
    chain_ids = set()
    updated_count = 0
    
    print(f"\n🔄 Updating chain_id to '{NEW_CHAIN_ID}' for all thoughts...")
    
    for key in thought_keys:
        try:
            # Get the thought data
            thought_json = r.json().get(key)
            
            if thought_json:
                # Track original chain_id
                if 'chain_id' in thought_json:
                    chain_ids.add(thought_json['chain_id'])
                
                # Update chain_id
                thought_json['chain_id'] = NEW_CHAIN_ID
                
                # Save back to Redis
                r.json().set(key, '$', thought_json)
                updated_count += 1
                
                # Show progress
                if updated_count % 10 == 0:
                    print(f"  Updated {updated_count}/{len(thought_keys)} thoughts...")
        
        except Exception as e:
            print(f"  ⚠️  Failed to update {key}: {e}")
    
    print(f"\n✅ Updated {updated_count} thoughts with chain_id: '{NEW_CHAIN_ID}'")
    
    # Create/update chain record
    chain_key = f"{INSTANCE_ID}:chains:{NEW_CHAIN_ID}"
    
    # Extract thought IDs from keys
    thought_ids = [key.split(":")[-1] for key in thought_keys]
    
    chain_data = {
        "chain_id": NEW_CHAIN_ID,
        "thought_ids": thought_ids,
        "created_at": datetime.utcnow().isoformat() + "Z",
        "description": "Consolidated chain for initial comprehensive summary",
        "original_chains": list(chain_ids)
    }
    
    print(f"\n📝 Creating chain record: {chain_key}")
    r.json().set(chain_key, '$', chain_data)
    print(f"✅ Chain record created with {len(thought_ids)} thoughts")
    
    # Show summary
    print("\n" + "="*60)
    print("📊 CONSOLIDATION SUMMARY")
    print("="*60)
    print(f"  Instance ID: {INSTANCE_ID}")
    print(f"  New Chain ID: {NEW_CHAIN_ID}")
    print(f"  Thoughts Updated: {updated_count}")
    print(f"  Original Chains: {len(chain_ids)}")
    if chain_ids:
        for cid in list(chain_ids)[:5]:  # Show first 5
            print(f"    - {cid}")
        if len(chain_ids) > 5:
            print(f"    ... and {len(chain_ids) - 5} more")
    
    print("\n✨ Next Steps:")
    print("1. Update Warp's KG entity with current_session_chain_id = 'initial-summary'")
    print("2. Call ui_start with user='Warp' to generate comprehensive summary")
    
    # Optionally, update the KG entity directly
    print("\n🔍 Looking for Warp's KG entity...")
    kg_pattern = "kg:federation:entities:*"
    kg_keys = list(r.scan_iter(match=kg_pattern))
    
    warp_entity = None
    for key in kg_keys:
        try:
            entity = r.json().get(key)
            if entity and entity.get('name') == 'Warp':
                warp_entity = (key, entity)
                break
        except:
            pass
    
    if warp_entity:
        key, entity = warp_entity
        print(f"✅ Found Warp entity: {key}")
        
        # Update current_session_chain_id
        if 'attributes' not in entity:
            entity['attributes'] = {}
        
        old_chain = entity['attributes'].get('current_session_chain_id', 'None')
        entity['attributes']['current_session_chain_id'] = NEW_CHAIN_ID
        
        # Save back
        r.json().set(key, '$', entity)
        print(f"✅ Updated Warp's current_session_chain_id: {old_chain} → {NEW_CHAIN_ID}")
        print("\n🚀 Ready! You can now call: ui_start with user='Warp'")
    else:
        print("⚠️  Could not find Warp's KG entity - you'll need to update it manually")

if __name__ == "__main__":
    main()
