# Implementation Plan: UnifiedIntelligence MCP Improvements (20250723)

This document outlines a detailed plan for implementing the improvements suggested in `unifiedintelligence.md`. The plan is divided into phases, starting with the most critical fixes and progressing to architectural enhancements and code cleanup.

## Phase 1: Critical Fixes

### 1.1. Implement `ui_recall`

*   **Description:** The `ui_recall` tool is a core feature for memory retrieval but is currently not implemented. This task involves creating the full functionality for this tool, allowing the agent to retrieve specific thoughts by their ID.
*   **Affected Files:**
    *   `src/handlers/mod.rs`
    *   `src/handlers/recall.rs` (new file)
    *   `src/repository.rs`
    *   `src/main.rs`
*   **Implementation Steps:**
    1.  Create a new file `src/handlers/recall.rs`.
    2.  Define a public asynchronous `recall` function within this new file that accepts the recall parameters.
    3.  Implement the core logic to parse and validate the `thought_id` from the input.
    4.  Add a new function to the `ThoughtRepository` trait in `src/repository_traits.rs` called `get_thought_by_id(thought_id: &str) -> Result<Option<ThoughtRecord>, Error>`.
    5.  Implement the `get_thought_by_id` function in `src/repository.rs` to fetch the corresponding Redis Hash.
    6.  Call the repository function from the `recall` handler.
    7.  Format the retrieved `ThoughtRecord` into a user-friendly string for output.
    8.  Add a `recall` module to `src/handlers/mod.rs`.
    9.  In `src/main.rs`, add a new match arm for `ui_recall` in the tool dispatch logic to call the new handler.
*   **Verification:**
    1.  Add unit tests for the `recall` handler, mocking the repository layer.
    2.  Add an integration test in the `tests/` directory that first uses `ui_think` to store a thought and then `ui_recall` to retrieve it, asserting the content is identical.

### 1.2. Transactionality in `ui_think`

*   **Description:** The `ui_think` operation currently performs multiple, non-atomic Redis commands. This can lead to data inconsistency if one command fails. This task is to wrap these operations in a Redis transaction (`MULTI`/`EXEC`).
*   **Affected Files:**
    *   `src/handlers/thoughts.rs`
    *   `src/redis.rs` or `src/repository.rs`
*   **Implementation Steps:**
    1.  In `src/redis.rs`, investigate the `redis-rs` library's support for transactions. The `pipe()` function is the standard way to achieve this.
    2.  Refactor the `save_thought` and related metadata-saving logic in `src/repository.rs`.
    3.  Create a new function, e.g., `save_thought_transaction`, that encapsulates the multiple Redis commands.
    4.  Inside this function, start a pipeline using `redis::pipe()`.
    5.  Add all the necessary commands (`HSET` for the thought, `HSET` for metadata, `LPUSH` to a chain list, etc.) to the pipeline.
    6.  Execute the pipeline atomically using the repository's Redis connection.
    7.  Update the `think` handler in `src/handlers/thoughts.rs` to call this new transactional function.
*   **Verification:**
    1.  Write a new integration test that specifically targets this transactionality. This can be done by attempting a `ui_think` operation and using a mock Redis server that is programmed to fail on one of the commands in the sequence.
    2.  After the failed operation, connect to the test Redis instance and verify that no partial data (e.g., an orphaned thought record without its metadata) was persisted.

### 1.3. Enhance Input Sanitization

*   **Description:** To improve security, all user-provided inputs must be rigorously sanitized to prevent potential injection attacks, particularly for search queries.
*   **Affected Files:**
    *   `src/validation.rs`
    *   All handler files in `src/handlers/`
*   **Implementation Steps:**
    1.  Review all structs in `src/models.rs` that are deserialized from user input.
    2.  In `src/validation.rs`, create a new function `sanitize_string(input: &str) -> String` that performs actions like trimming whitespace.
    3.  For inputs used in RediSearch queries (like `ui_search`'s `query` parameter), create a specific `sanitize_search_query` function that escapes special characters used by the RediSearch engine (e.g., `@`, `!`, `-`).
    4.  Apply these sanitization functions within each handler immediately after deserializing the input parameters.
*   **Verification:**
    1.  Add unit tests to `src/validation.rs` that test the sanitization functions with a variety of inputs, including strings with special characters, leading/trailing whitespace, and potential injection payloads.
    2.  Create integration tests that use `ui_search` with sanitized special characters and assert that the search executes safely without errors and returns the correct results.

## Phase 2: Architecture Improvements

### 2.1. Decouple Handlers from Repository via Service Layer

*   **Description:** Introduce a service layer to abstract business logic away from the handlers, improving separation of concerns and testability.
*   **Affected Files:**
    *   `src/handlers/*.rs`
    *   `src/repository.rs`
    *   `src/service.rs` (new file)
    *   `src/lib.rs` (to declare the new module)
*   **Implementation Steps:**
    1.  Create a new file `src/service.rs` and declare it in `src/lib.rs`.
    2.  Define a `ThoughtService` struct within `service.rs`. This struct will hold an instance of the `ThoughtRepository`.
    3.  Move the complex business logic currently in the `think` handler (e.g., creating the `ThoughtRecord`, managing chain logic, preparing metadata) into methods on the `ThoughtService`.
    4.  Refactor the `think` handler to be leaner. It should now primarily parse the request, call the appropriate `ThoughtService` method, and format the response.
    5.  Update the creation of the handler in `main.rs` to inject the `ThoughtService`.
*   **Verification:**
    1.  Ensure all existing unit and integration tests pass after this significant refactoring.
    2.  Add new unit tests specifically for the `ThoughtService`, mocking the repository to test the business logic in isolation.

### 2.2. Implement Advanced Search Capabilities

*   **Description:** Enhance the `ui_search` tool to support more powerful queries, including filtering by date ranges and sorting by metadata fields like importance.
*   **Affected Files:**
    *   `src/handlers/search.rs`
    *   `src/repository.rs`
    *   `src/models.rs`
*   **Implementation Steps:**
    1.  **Update Search Index:** Modify the `FT.CREATE` command for the thought index to make `timestamp`, `importance`, and `relevance` fields `SORTABLE`.
    2.  **Extend Search Parameters:** Add optional `sort_by`, `sort_order`, `date_start`, and `date_end` fields to the `SearchParameters` struct in `src/models.rs`.
    3.  **Dynamic Query Building:** In `src/repository.rs`, refactor the search function to dynamically build the RediSearch query.
        *   If `date_start` or `date_end` are provided, add a numeric range filter on the `timestamp` field (e.g., `FILTER timestamp <start> <end>`).
        *   If `sort_by` is provided, add the `SORTBY` clause to the query.
    4.  **Update Handler:** The `search` handler will pass these new parameters to the repository.
*   **Verification:**
    1.  Add new integration tests that store multiple thoughts with different timestamps and importance scores.
    2.  Write new tests for `ui_search` that use the new sorting and date filtering parameters and assert that the results are returned in the correct order and within the specified date range.

## Phase 3: Code Cleanup

### 3.1. Review and Remove Unused Code

*   **Description:** The project contains a number of one-off scripts and potentially abandoned test files. These should be reviewed, and any non-essential files should be removed to clean up the codebase.
*   **Affected Files:**
    *   `clean-up/` directory
    *   `test_duplicate_thoughts.py`
*   **Implementation Steps:**
    1.  Thoroughly examine the contents of the `clean-up/` directory. Identify any scripts that were for temporary debugging or one-time data migration.
    2.  Create an `archive/` directory and move any scripts that might have future value into it.
    3.  Delete the now-empty `clean-up/` directory and the archived files from the git history if desired.
    4.  Delete the standalone `test_duplicate_thoughts.py` script.
    5.  Perform a global search for any significant blocks of commented-out code and remove them.
*   **Verification:**
    1.  After removing the files, run the full test suite (`cargo test`) to ensure that no critical dependencies were accidentally removed.
    2.  Manually build the project (`cargo build --release`) to confirm it still compiles correctly.
