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
    for i in range(len(lines)):
        if i >= 180 and i <= 367:
            remove_lines.add(i)
    
    # Find action implementations section (lines 369-629)
    for i in range(len(lines)):
        if i >= 368 and i <= 628:
            remove_lines.add(i)
    
    # Find ui_debug_env handler (lines 1217-1244)
    for i in range(len(lines)):
        if i >= 1216 and i <= 1243:
            remove_lines.add(i)
    
    # Find ui_recall_feedback handler (lines 1246-1299)
    for i in range(len(lines)):
        if i >= 1245 and i <= 1298:
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

if __name__ == '__main__':
    main()