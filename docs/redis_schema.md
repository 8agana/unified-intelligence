# Redis Key Schema

This document outlines the Redis key schema used by the Unified Intelligence application. A consistent key schema is crucial for avoiding collisions, organizing data, and simplifying debugging.

## Key Naming Convention

The general convention for keys is `{instance}:{type}:{id}`.

-   `{instance}`: An identifier for the user or context (e.g., `DT`, `CC`). This provides a namespace for all data related to a specific instance.
-   `{type}`: The type of data being stored (e.g., `thought`, `chain`, `event`).
-   `{id}`: A unique identifier for the specific data entry.

---

## Key Schemas

### 1. Embeddings

-   **Key Pattern:** `embedding:{sha256_hash}`
-   **Type:** `String`
-   **Description:** Caches the vector embedding for a given text. The text is hashed using SHA256 to create a deterministic key. The value is the raw binary representation of the `Vec<f32>` embedding, serialized using `bincode`.
-   **Example Key:** `embedding:1a79a4d60de6718e8e5b326e338ae53344224435542577435353554252442a`
-   **Managed in:** `src/redis.rs` (`get_cached_embedding`, `set_cached_embedding`)

### 2. Event Streams

-   **Key Pattern:** `{instance}:events`
-   **Type:** `Stream`
-   **Description:** A Redis Stream used to log various events that occur within the application for a specific instance. This can be used for auditing, analytics, or debugging. The stream has a maximum length to prevent it from growing indefinitely.
-   **Example Key:** `DT:events`
-   **Managed in:** `src/redis.rs` (`init_event_stream`, `log_event`, etc.)

### 3. Thoughts

-   **Key Pattern:** `{instance}:Thoughts:{thought_id}`
-   **Type:** `JSON`
-   **Description:** Stores a single `ThoughtRecord` object as a JSON document.
-   **Example Key:** `DT:Thoughts:a1b2c3d4-e5f6-7890-1234-567890abcdef`
-   **Managed in:** `src/repository.rs`

### 4. Chains

-   **Key Pattern (Metadata):** `Chains:metadata:{chain_id}`
-   **Type:** `JSON`
-   **Description:** Stores the `ChainMetadata` for a specific chain of thoughts.
-   **Note:** This key is currently inconsistent as it lacks the `{instance}` prefix.
-   **Example Key:** `Chains:metadata:c1d2e3f4-a5b6-7890-1234-567890abcdef`
-   **Managed in:** `src/repository.rs`

-   **Key Pattern (Thoughts List):** `{instance}:chains:{chain_id}`
-   **Type:** `Set` or `List` (managed by Lua script)
-   **Description:** Stores the set of thought IDs that belong to a specific chain, enabling retrieval of all thoughts in a chain.
-   **Example Key:** `DT:chains:c1d2e3f4-a5b6-7890-1234-567890abcdef`
-   **Managed in:** `src/repository.rs` (via Lua script)

### 5. Metrics and Filters

-   **Key Pattern (Bloom Filter):** `{instance}:bloom:thoughts`
-   **Type:** `String` (used by `BF.ADD`/`BF.EXISTS`)
-   **Description:** A Bloom filter to quickly check for the existence of a thought, helping to prevent duplicate thought processing.
-   **Example Key:** `DT:bloom:thoughts`
-   **Managed in:** `src/repository.rs` (via Lua script)

-   **Key Pattern (Thought Counter):** `{instance}:metrics:thought_count`
-   **Type:** `String` (used as a counter)
-   **Description:** A counter for the total number of thoughts for a given instance.
-   **Example Key:** `DT:metrics:thought_count`
-   **Managed in:** `src/repository.rs` (via Lua script)

### 6. RediSearch Index

-   **Key Pattern:** `{instance}:thoughts_idx`
-   **Description:** The name of the RediSearch index for thoughts, used to perform full-text searches.
-   **Example Key:** `DT:thoughts_idx`
-   **Managed in:** `src/repository.rs`

### 7. Knowledge Graph

-   **Entity Key:** `{prefix}:KG:entity:{id}`
-   **Relation Key:** `{prefix}:KG:relation:{id}`
-   **Name Index Key:** `{prefix}:KG:index:name_to_id` (Type: `Hash`)
-   **Relation Index Key:** `{prefix}:KG:index:entity_relations:{entity_id}` (Type: `Hash`)
-   **Description:** A set of keys for storing a knowledge graph. `{prefix}` is either the instance ID or a global scope name. Entities and relations are stored as JSON, while indexes are stored as Hashes for efficient lookups.
-   **Managed in:** `src/repository.rs` (`RedisKnowledgeRepository`)
