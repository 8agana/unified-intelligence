#!/usr/bin/env python3
"""Remove specific handlers from handlers.rs - Version 2"""

def find_function_end(lines, start_idx):
    """Find the end of a function starting at start_idx"""
    brace_count = 0
    in_function = False
    
    for i in range(start_idx, len(lines)):
        line = lines[i]
        
        # Count braces
        for char in line:
            if char == '{':
                brace_count += 1
                in_function = True
            elif char == '}':
                brace_count -= 1
        
        # Function ends when we return to brace_count 0
        if in_function and brace_count == 0:
            return i
    
    return len(lines) - 1

def main():
    with open('src/handlers.rs', 'r') as f:
        lines = f.readlines()
    
    # Find and mark sections to remove
    remove_ranges = []
    
    # Find ui_recall handler
    for i, line in enumerate(lines):
        if 'pub async fn ui_recall(&self, params: UiRecallParams)' in line:
            end = find_function_end(lines, i)
            remove_ranges.append((i-1, end))  # Include comment line before
            print(f"Found ui_recall: lines {i} to {end}")
            break
    
    # Find helper functions
    for i, line in enumerate(lines):
        if '// Action implementations' in line.strip():
            # Find next major section
            for j in range(i+1, len(lines)):
                if '// Helper methods for document-based identity operations' in lines[j]:
                    remove_ranges.append((i, j-1))
                    print(f"Found action implementations: lines {i} to {j-1}")
                    break
            break
    
    # Find ui_debug_env handler
    for i, line in enumerate(lines):
        if 'pub async fn ui_debug_env(&self, _params: UiDebugEnvParams)' in line:
            end = find_function_end(lines, i)
            remove_ranges.append((i-1, end))  # Include comment line before
            print(f"Found ui_debug_env: lines {i} to {end}")
            break
    
    # Find ui_recall_feedback handler
    for i, line in enumerate(lines):
        if 'pub async fn ui_recall_feedback(&self, params: UiRecallFeedbackParams)' in line:
            end = find_function_end(lines, i)
            remove_ranges.append((i-1, end))  # Include comment line before
            print(f"Found ui_recall_feedback: lines {i} to {end}")
            break
    
    # Build output skipping remove ranges
    output_lines = []
    skip_ranges = sorted(remove_ranges)
    
    i = 0
    while i < len(lines):
        skip = False
        for start, end in skip_ranges:
            if start <= i <= end:
                skip = True
                i = end + 1
                break
        
        if not skip:
            output_lines.append(lines[i])
            i += 1
    
    # Write the cleaned file
    with open('src/handlers_cleaned.rs', 'w') as f:
        f.writelines(output_lines)
    
    print(f"\nOriginal file: {len(lines)} lines")
    print(f"Cleaned file: {len(output_lines)} lines")
    print(f"Removed: {len(lines) - len(output_lines)} lines")

if __name__ == '__main__':
    main()