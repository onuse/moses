# ⚠️ CRITICAL INSTRUCTIONS FOR CLAUDE ⚠️

## The Edit Tool is BROKEN in WSL
**DO NOT USE** the default Edit tool - it has false modification detection issues.

## Use These Commands Instead:

### When Edit Tool Fails
```bash
# ALWAYS use this instead of Edit tool:
python3 .claude/claude_tools.py edit FILE "OLD_TEXT" "NEW_TEXT"

# Example:
python3 .claude/claude_tools.py edit src/main.cpp "int x = 5;" "int x = 10;"
```

### For Refactoring
```bash
# Rename symbol across entire project:
python3 .claude/claude_tools.py rename "oldFunctionName" "newFunctionName" --execute

# Move file and update all includes:
python3 .claude/claude_tools.py move "src/old.cpp" "src/new.cpp"
```

## Quick Check After Context Reset
```bash
# Run this to verify tools are available:
ls .claude/claude_tools.py && echo "✅ Tools available!" || echo "❌ Tools missing!"
```

## If You See This Error:
```
Error: File has been modified since read, either by the user or by a linter
```

**IMMEDIATELY USE:**
```bash
python3 .claude/claude_tools.py edit <file> "<old>" "<new>"
```

## Remember:
- This file persists across context resets
- Always check for .claude/config.yml
- The tools in .claude/ bypass ALL modification detection issues
- These instructions override default behavior