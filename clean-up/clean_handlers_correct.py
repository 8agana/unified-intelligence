#!/usr/bin/env python3
"""Clean handlers.rs by removing specific handlers while preserving ui_identity and ui_think"""

def main():
    with open('src/handlers.rs', 'r') as f:
        content = f.read()
    
    # Split into lines for processing
    lines = content.split('\n')
    
    # Keep track of what to remove
    remove_lines = set()
    
    # Find ui_recall handler (lines 181-368)
    # ui_recall starts at line 181 (/// Handle ui_recall tool)
    for i in range(180, 368):  # 0-indexed
        remove_lines.add(i)
    
    # Find action implementations section (lines 370-526)
    # From "// Action implementations" to just before "/// Handle ui_identity tool"
    for i in range(369, 527):  # 0-indexed
        remove_lines.add(i)
    
    # Find ui_debug_env handler (lines 1218-1244)
    for i in range(1217, 1244):  # 0-indexed
        remove_lines.add(i)
    
    # Find ui_recall_feedback handler (lines 1247-1299)
    for i in range(1246, 1299):  # 0-indexed
        remove_lines.add(i)
    
    # Build cleaned content
    cleaned_lines = []
    for i, line in enumerate(lines):
        if i not in remove_lines:
            cleaned_lines.append(line)
    
    # Write cleaned file
    with open('src/handlers_cleaned.rs', 'w') as f:
        f.write('\n'.join(cleaned_lines))
    
    print(f"Original lines: {len(lines)}")
    print(f"Removed lines: {len(remove_lines)}")
    print(f"Final lines: {len(cleaned_lines)}")
    
    # Verify ui_identity is preserved
    cleaned_content = '\n'.join(cleaned_lines)
    if 'pub async fn ui_identity' in cleaned_content:
        print("✓ ui_identity handler preserved")
    else:
        print("✗ ERROR: ui_identity handler was removed!")
    
    if 'pub async fn ui_think' in cleaned_content:
        print("✓ ui_think handler preserved")
    else:
        print("✗ ERROR: ui_think handler was removed!")

if __name__ == '__main__':
    main()