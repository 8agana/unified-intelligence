# Fallback Search Tests

This document describes the test suite for the `fallback_search` functionality in UnifiedIntelligence.

## Overview

The `fallback_search` methods provide a text-based search mechanism that is used when Redis Search (FT.SEARCH) is unavailable or fails. This is critical for ensuring search functionality remains available even in degraded conditions.

## Test Coverage

### Unit Tests (`tests/fallback_search_tests.rs`)

The test suite covers the following scenarios:

1. **Case-insensitive matching** - Verifies that searches work regardless of case
2. **Limit enforcement** - Ensures the limit parameter is respected
3. **Global search** - Tests searching across multiple instances
4. **Empty results** - Handles cases where no matches are found
5. **Partial word matching** - Substring matching works correctly
6. **Special characters** - Search works with punctuation and special chars

### Integration Tests (`src/repository.rs`)

Basic unit tests are also included in the repository module:

1. **Search matching logic** - Tests the core substring matching algorithm
2. **Limit enforcement** - Verifies that results are properly limited

## Test Structure

### Helper Functions

```rust
fn create_test_thought(id: &str, content: &str, instance: &str) -> ThoughtRecord
```
Creates test ThoughtRecord instances with proper structure.

### Test Categories

1. **Functional Tests** - Verify core search functionality
2. **Edge Case Tests** - Handle empty results, special characters
3. **Performance Tests** - Ensure limits are enforced
4. **Scenario Tests** - Document expected behavior in different scenarios

## Running the Tests

```bash
# Run all fallback search tests
cargo test --test fallback_search_tests

# Run with output
cargo test --test fallback_search_tests -- --nocapture

# Run specific test
cargo test test_case_insensitive_search_matching
```

## Implementation Notes

The fallback search implementation:
- Uses Redis SCAN to iterate through keys
- Attempts JSON.GET first, falls back to regular GET
- Performs case-insensitive substring matching
- Respects limit parameters
- Works for both instance-specific and global searches

## Future Improvements

1. Add integration tests with actual Redis instance
2. Add performance benchmarks for large datasets
3. Test concurrent access scenarios
4. Add tests for error recovery and retry logic