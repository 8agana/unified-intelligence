#!/usr/bin/env python3
"""Remove specific handlers from handlers.rs"""

def main():
    with open('src/handlers.rs', 'r') as f:
        lines = f.readlines()
    
    # Define sections to remove
    remove_sections = [
        {
            'start': '    /// Handle ui_recall tool (Phase 2 Enhanced)',
            'end': '    }',
            'end_line': 368
        },
        {
            'start': '    // Action implementations',
            'end': '    // Helper methods for document-based identity operations',
            'end_line': 631
        },
        {
            'start': '    /// Handle ui_debug_env tool - returns masked environment variables',
            'end': '    }',
            'end_line': 1244
        },
        {
            'start': '    /// Handle ui_recall_feedback tool - record feedback on search results (Phase 2)',
            'end': '    }',
            'end_line': 1297
        }
    ]
    
    output_lines = []
    i = 0
    
    while i < len(lines):
        skip = False
        
        for section in remove_sections:
            if lines[i].strip() == section['start'].strip():
                # Skip until we find the end
                while i < len(lines) and i <= section['end_line']:
                    i += 1
                skip = True
                break
        
        if not skip:
            output_lines.append(lines[i])
            i += 1
    
    # Write the cleaned file
    with open('src/handlers_cleaned.rs', 'w') as f:
        f.writelines(output_lines)
    
    print(f"Original file: {len(lines)} lines")
    print(f"Cleaned file: {len(output_lines)} lines")
    print(f"Removed: {len(lines) - len(output_lines)} lines")

if __name__ == '__main__':
    main()