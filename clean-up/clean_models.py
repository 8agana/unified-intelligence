#!/usr/bin/env python3
"""Remove unwanted parameter and response structs from models.rs"""

import re

def find_struct_end(lines, start_idx):
    """Find the end of a struct starting at start_idx"""
    brace_count = 0
    in_struct = False
    
    for i in range(start_idx, len(lines)):
        line = lines[i]
        
        # Count braces
        for char in line:
            if char == '{':
                brace_count += 1
                in_struct = True
            elif char == '}':
                brace_count -= 1
        
        # Struct ends when we return to brace_count 0
        if in_struct and brace_count == 0:
            return i
    
    return len(lines) - 1

def main():
    with open('src/models.rs', 'r') as f:
        lines = f.readlines()
    
    # Find structs to remove
    structs_to_remove = [
        'UiRecallParams',
        'UiDebugEnvParams', 
        'UiRecallFeedbackParams',
        'RecallResponse',
        'DebugEnvResponse',
        'FeedbackResponse'
    ]
    
    remove_ranges = []
    
    for struct_name in structs_to_remove:
        for i, line in enumerate(lines):
            if f'pub struct {struct_name}' in line:
                # Find the derive line before it (if any)
                start = i
                if i > 0 and '#[derive' in lines[i-1]:
                    start = i - 1
                    # Check if there's a doc comment before that
                    if i > 1 and '///' in lines[i-2]:
                        start = i - 2
                
                end = find_struct_end(lines, i)
                remove_ranges.append((start, end))
                print(f"Found {struct_name}: lines {start+1} to {end+1}")
                break
    
    # Sort ranges by start position in reverse order
    remove_ranges.sort(reverse=True)
    
    # Remove in reverse order to maintain line numbers
    for start, end in remove_ranges:
        # Remove the struct and any blank line after it
        if end + 1 < len(lines) and lines[end + 1].strip() == '':
            del lines[start:end+2]
        else:
            del lines[start:end+1]
    
    # Write cleaned file
    with open('src/models_cleaned.rs', 'w') as f:
        f.writelines(lines)
    
    print(f"\nCleaned models.rs created")

if __name__ == '__main__':
    main()