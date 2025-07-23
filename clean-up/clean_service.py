#!/usr/bin/env python3
"""Remove unwanted tool registrations from service.rs"""

def main():
    with open('src/service.rs', 'r') as f:
        lines = f.readlines()
    
    # Keep track of what to remove
    remove_lines = set()
    
    # Find ui_recall tool registration
    for i in range(len(lines)):
        if '#[tool(description = "Search, retrieve, and manipulate stored thoughts")]' in lines[i]:
            # Remove from this line until the closing brace
            start = i
            brace_count = 0
            in_function = False
            for j in range(i, len(lines)):
                line = lines[j]
                for char in line:
                    if char == '{':
                        brace_count += 1
                        in_function = True
                    elif char == '}':
                        brace_count -= 1
                if in_function and brace_count == 0:
                    for k in range(start, j+1):
                        remove_lines.add(k)
                    break
    
    # Find ui_recall_feedback tool registration
    for i in range(len(lines)):
        if '#[tool(description = "Record feedback on search results to improve future searches")]' in lines[i]:
            # Remove from this line until the closing brace
            start = i
            brace_count = 0
            in_function = False
            for j in range(i, len(lines)):
                line = lines[j]
                for char in line:
                    if char == '{':
                        brace_count += 1
                        in_function = True
                    elif char == '}':
                        brace_count -= 1
                if in_function and brace_count == 0:
                    for k in range(start, j+1):
                        remove_lines.add(k)
                    break
    
    # Find ui_debug_env tool registration
    for i in range(len(lines)):
        if '#[tool(description = "Debug tool to view masked environment variables' in lines[i]:
            # Remove from this line until the closing brace
            start = i
            brace_count = 0
            in_function = False
            for j in range(i, len(lines)):
                line = lines[j]
                for char in line:
                    if char == '{':
                        brace_count += 1
                        in_function = True
                    elif char == '}':
                        brace_count -= 1
                if in_function and brace_count == 0:
                    for k in range(start, j+1):
                        remove_lines.add(k)
                    break
    
    # Build cleaned content
    cleaned_lines = []
    for i, line in enumerate(lines):
        if i not in remove_lines:
            cleaned_lines.append(line)
    
    # Write cleaned file
    with open('src/service_cleaned.rs', 'w') as f:
        f.writelines(cleaned_lines)
    
    print(f"Original lines: {len(lines)}")
    print(f"Removed lines: {len(remove_lines)}")
    print(f"Final lines: {len(cleaned_lines)}")

if __name__ == '__main__':
    main()