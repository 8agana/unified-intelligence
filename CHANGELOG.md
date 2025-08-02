# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Unified metadata architecture for thought storage
- Display trait implementation for ThinkingFramework enum
- Flexible integer deserialization for MCP client compatibility (DT, CC, Warp)
- Complete metadata embedding pipeline to Qdrant
- Support for framework, importance, relevance, category, and tags in thought records

### Changed
- Updated Rust edition to 2024
- Unified ThoughtRecord structure to include all metadata fields
- Replaced split ThoughtRecord/ThoughtMetadata architecture with single unified record
- Modified embedding pipeline to use unified ThoughtRecord from unified_intelligence crate
- Updated all ThoughtRecord::new calls to include metadata parameters

### Deprecated
- Split ThoughtRecord/ThoughtMetadata architecture (replaced with unified structure)

### Removed
- Separate ThoughtMetadata storage in Redis (now unified in ThoughtRecord)
- External dependency on separate metadata structures in embedding pipeline

### Fixed
- Critical metadata embedding failure - metadata now properly flows to Qdrant
- Self-dependency compilation errors by adding missing module declarations
- ThinkingFramework Display trait implementation for framework string conversion
- MCP client compatibility issues with integer parameter serialization
- Auto-generated thought creation calls to include all required metadata fields
- Ensured `icon` variable is used in `display_framework_start` function
- Removed "sequential" from framework validation error message (framework not actually implemented)

### Security
- 

## [2.0.0] - 2025-08-01
### Added
- Initial release of unified-intelligence (Model Context Protocol MCP) using rmcp 0.3.0.

```
- Replace, add, or remove sections as appropriate for your project.
- For each new release, copy the "Unreleased" section, change the version/date, and start a new "Unreleased" section.

