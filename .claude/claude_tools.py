#!/usr/bin/env python3
"""
Claude Tools - Proof of Concept
Enhanced editing tools for AI-assisted development
"""

import os
import re
import json
import shutil
from pathlib import Path
from typing import List, Dict, Optional, Union, Tuple
from dataclasses import dataclass
from contextlib import contextmanager
import subprocess

@dataclass
class FileChange:
    """Record of a file change for rollback"""
    file_path: str
    backup_path: str
    operation: str
    
@dataclass
class ToolResult:
    """Standard result from any tool operation"""
    success: bool
    message: str
    changes: List[FileChange] = None
    warnings: List[str] = None
    
class ClaudeTools:
    def __init__(self, backup_dir=".claude_backups"):
        self.backup_dir = Path(backup_dir)
        self.backup_dir.mkdir(exist_ok=True)
        self.transaction_changes = []
        self.in_transaction = False
        
    # ==================== Core Editing Tools ====================
    
    def safe_edit(self, file_path: str, changes: List[Dict]) -> ToolResult:
        """
        Edit file without modification checks
        changes = [{"old": "text", "new": "text", "all": False}]
        """
        try:
            backup = self._backup_file(file_path)
            
            with open(file_path, 'r') as f:
                content = f.read()
            
            original = content
            for change in changes:
                old_text = change["old"]
                new_text = change["new"]
                replace_all = change.get("all", False)
                occurrence = change.get("occurrence", 1)
                
                if old_text not in content:
                    return ToolResult(False, f"Pattern not found: {old_text[:50]}...")
                
                if replace_all:
                    content = content.replace(old_text, new_text)
                else:
                    # Replace specific occurrence
                    parts = content.split(old_text)
                    if len(parts) > occurrence:
                        content = old_text.join(parts[:occurrence]) + new_text + old_text.join(parts[occurrence:])
            
            with open(file_path, 'w') as f:
                f.write(content)
            
            change_record = FileChange(file_path, backup, "edit")
            if self.in_transaction:
                self.transaction_changes.append(change_record)
                
            return ToolResult(True, f"Successfully edited {file_path}", [change_record])
            
        except Exception as e:
            return ToolResult(False, f"Error: {str(e)}")
    
    def bulk_edit(self, operations: List[Dict]) -> ToolResult:
        """Execute multiple edits atomically"""
        all_changes = []
        
        try:
            for op in operations:
                result = self.safe_edit(op["file"], op.get("changes", [{"old": op["old"], "new": op["new"]}]))
                if not result.success:
                    # Rollback all changes
                    for change in all_changes:
                        shutil.copy2(change.backup_path, change.file_path)
                    return ToolResult(False, f"Failed at {op['file']}: {result.message}")
                all_changes.extend(result.changes)
                
            return ToolResult(True, f"Successfully edited {len(operations)} files", all_changes)
            
        except Exception as e:
            return ToolResult(False, f"Bulk edit error: {str(e)}")
    
    # ==================== Refactoring Tools ====================
    
    def rename_symbol(self, old_name: str, new_name: str, 
                     file_pattern: str = "**/*", 
                     preview: bool = True) -> ToolResult:
        """Rename a symbol across multiple files"""
        import glob
        
        files_to_change = []
        total_occurrences = 0
        
        # Find all matching files
        for file_path in Path(".").glob(file_pattern):
            if file_path.is_file() and not str(file_path).startswith(('.git', 'build', '__pycache__')):
                try:
                    with open(file_path, 'r') as f:
                        content = f.read()
                    
                    # Use word boundaries for smarter matching
                    pattern = r'\b' + re.escape(old_name) + r'\b'
                    matches = len(re.findall(pattern, content))
                    
                    if matches > 0:
                        files_to_change.append((str(file_path), matches))
                        total_occurrences += matches
                except:
                    continue
        
        if preview:
            preview_msg = f"Would rename '{old_name}' to '{new_name}':\n"
            preview_msg += f"  {total_occurrences} occurrences in {len(files_to_change)} files:\n"
            for file, count in files_to_change[:10]:  # Show first 10
                preview_msg += f"    {file}: {count} occurrences\n"
            if len(files_to_change) > 10:
                preview_msg += f"    ... and {len(files_to_change)-10} more files\n"
            return ToolResult(True, preview_msg)
        
        # Perform actual rename
        all_changes = []
        for file_path, _ in files_to_change:
            pattern = r'\b' + re.escape(old_name) + r'\b'
            result = self.safe_edit(file_path, [{"old": old_name, "new": new_name, "all": True}])
            if not result.success:
                # Rollback
                for change in all_changes:
                    shutil.copy2(change.backup_path, change.file_path)
                return result
            all_changes.extend(result.changes)
        
        return ToolResult(True, 
                         f"Renamed {total_occurrences} occurrences in {len(files_to_change)} files",
                         all_changes)
    
    def safe_move(self, from_path: str, to_path: str, update_includes: bool = True) -> ToolResult:
        """Move file and update references"""
        try:
            from_p = Path(from_path)
            to_p = Path(to_path)
            
            if not from_p.exists():
                return ToolResult(False, f"Source file {from_path} not found")
            
            # Backup original
            backup = self._backup_file(str(from_p))
            
            # Create target directory if needed
            to_p.parent.mkdir(parents=True, exist_ok=True)
            
            # Move the file
            shutil.move(str(from_p), str(to_p))
            
            changes = [FileChange(str(from_p), backup, "move")]
            
            # Update includes if requested
            if update_includes:
                old_include = f'#include "{from_p.name}"'
                new_include = f'#include "{to_p.name}"'
                
                # Find and update includes
                include_updates = self.find_replace_all(
                    pattern=old_include,
                    replacement=new_include,
                    file_pattern="**/*.{cpp,hpp,h,c}",
                    dry_run=False
                )
                
                if include_updates.success and include_updates.changes:
                    changes.extend(include_updates.changes)
            
            return ToolResult(True, f"Moved {from_path} to {to_path}", changes)
            
        except Exception as e:
            return ToolResult(False, f"Move failed: {str(e)}")
    
    # ==================== Search Tools ====================
    
    def find_replace_all(self, pattern: str, replacement: str, 
                        file_pattern: str = "**/*", 
                        dry_run: bool = True) -> ToolResult:
        """Find and replace across multiple files"""
        import glob
        
        files_to_change = []
        total_matches = 0
        
        for file_path in Path(".").glob(file_pattern):
            if file_path.is_file() and not str(file_path).startswith(('.git', 'build')):
                try:
                    with open(file_path, 'r') as f:
                        content = f.read()
                    
                    matches = len(re.findall(pattern, content))
                    if matches > 0:
                        files_to_change.append((str(file_path), matches))
                        total_matches += matches
                except:
                    continue
        
        if dry_run:
            preview = f"Would replace {total_matches} occurrences in {len(files_to_change)} files"
            return ToolResult(True, preview)
        
        # Perform replacements
        all_changes = []
        for file_path, _ in files_to_change:
            backup = self._backup_file(file_path)
            
            with open(file_path, 'r') as f:
                content = f.read()
            
            new_content = re.sub(pattern, replacement, content)
            
            with open(file_path, 'w') as f:
                f.write(new_content)
            
            all_changes.append(FileChange(file_path, backup, "replace"))
        
        return ToolResult(True, 
                         f"Replaced {total_matches} occurrences in {len(files_to_change)} files",
                         all_changes)
    
    # ==================== Transaction Support ====================
    
    @contextmanager
    def transaction(self):
        """Context manager for atomic operations"""
        self.in_transaction = True
        self.transaction_changes = []
        
        try:
            yield self
            # Success - keep changes
            self.in_transaction = False
            
        except Exception as e:
            # Rollback all changes
            for change in reversed(self.transaction_changes):
                try:
                    if change.operation in ["edit", "replace"]:
                        shutil.copy2(change.backup_path, change.file_path)
                    elif change.operation == "move":
                        shutil.move(change.backup_path, change.file_path)
                except:
                    pass
            
            self.in_transaction = False
            raise e
        
        finally:
            self.transaction_changes = []
    
    # ==================== Utility Functions ====================
    
    def _backup_file(self, file_path: str) -> str:
        """Create backup of file"""
        import uuid
        backup_name = f"{Path(file_path).name}.{uuid.uuid4().hex[:8]}.bak"
        backup_path = self.backup_dir / backup_name
        shutil.copy2(file_path, backup_path)
        return str(backup_path)
    
    def path_convert(self, path: str, to: str = "wsl") -> str:
        """Convert between WSL and Windows paths"""
        if to == "windows":
            if path.startswith("/mnt/"):
                # /mnt/c/Users/... -> C:\Users\...
                parts = path.split("/")[2:]  # Skip /mnt/
                drive = parts[0].upper()
                return f"{drive}:\\" + "\\".join(parts[1:])
        else:  # to wsl
            if re.match(r"^[A-Za-z]:\\", path):
                # C:\Users\... -> /mnt/c/Users/...
                drive = path[0].lower()
                path_part = path[3:].replace("\\", "/")
                return f"/mnt/{drive}/{path_part}"
        return path


# ==================== CLI Interface ====================

if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser(description="Claude Tools - Enhanced editing for AI")
    subparsers = parser.add_subparsers(dest="command")
    
    # safe_edit command
    edit_parser = subparsers.add_parser("edit", help="Edit file without checks")
    edit_parser.add_argument("file", help="File to edit")
    edit_parser.add_argument("old", help="Text to find")
    edit_parser.add_argument("new", help="Text to replace with")
    edit_parser.add_argument("--all", action="store_true", help="Replace all occurrences")
    
    # rename command
    rename_parser = subparsers.add_parser("rename", help="Rename symbol across project")
    rename_parser.add_argument("old_name", help="Current name")
    rename_parser.add_argument("new_name", help="New name")
    rename_parser.add_argument("--pattern", default="**/*", help="File pattern")
    rename_parser.add_argument("--execute", action="store_true", help="Execute (not just preview)")
    
    # move command
    move_parser = subparsers.add_parser("move", help="Move file and update references")
    move_parser.add_argument("from_path", help="Source file")
    move_parser.add_argument("to_path", help="Destination")
    move_parser.add_argument("--no-update", action="store_true", help="Don't update includes")
    
    args = parser.parse_args()
    tools = ClaudeTools()
    
    if args.command == "edit":
        result = tools.safe_edit(args.file, [{"old": args.old, "new": args.new, "all": args.all}])
        print(result.message)
        
    elif args.command == "rename":
        result = tools.rename_symbol(args.old_name, args.new_name, 
                                    args.pattern, preview=not args.execute)
        print(result.message)
        
    elif args.command == "move":
        result = tools.safe_move(args.from_path, args.to_path, 
                               update_includes=not args.no_update)
        print(result.message)
    
    else:
        parser.print_help()