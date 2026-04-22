#!/usr/bin/env python3
"""
Bazel to Redwood BUILD.datalog Generator

Converts Bazel's compile_commands.json to BUILD.datalog format.

Usage:
    # Generate compile_commands.json from Bazel:
    bazel run @hedronv//:refresh_compile_commands
    # Or: https://github.com/grailbio/bazel-compilation-database

    # Then convert:
    python3 bazel_to_redwood.py compile_commands.json > BUILD.datalog

Note: Bazel's compile_commands.json often contains sandbox paths like:
  bazel-out/k8-fastbuild/bin/...
  execroot/__main__/...

This script normalizes these to workspace-relative paths.
"""

import json
import os
import sys
import re
from pathlib import Path
from typing import Dict, List, Set, Tuple


def escape_datalog_string(s: str) -> str:
    """Escape a string for Datalog syntax."""
    return s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n')


def normalize_bazel_path(path: str, workspace_root: str) -> str:
    """
    Normalize Bazel-specific paths to workspace-relative paths.

    Bazel uses paths like:
    - bazel-out/k8-fastbuild/bin/path/to/file
    - execroot/__main__/path/to/file
    - external/some_dep/path/to/file
    """
    path = os.path.normpath(path)

    # Remove bazel-out prefix (generated files)
    if 'bazel-out' in path:
        return path

    if 'execroot' in path:
        parts = path.split('execroot/')
        if len(parts) > 1:
            # execroot/__main__/src/foo.cc -> src/foo.cc
            after_execroot = parts[1]
            # Skip workspace name
            if '/' in after_execroot:
                path = '/'.join(after_execroot.split('/')[1:])

    if path.startswith('external/'):
        return path

    try:
        if os.path.isabs(path):
            return os.path.relpath(path, workspace_root)
    except ValueError:
        pass

    return path


def parse_bazel_compile_command(cmd: Dict, workspace_root: str) -> Tuple[str, str, List[str], str, str]:
    """
    Parse a Bazel compile command entry.
    Returns: (compiler, source_file, flags, output_file, directory)
    """
    source = cmd.get('file', '')
    directory = cmd.get('directory', workspace_root)
    command = cmd.get('command', '')
    output = cmd.get('output', '')

    import shlex
    parts = shlex.split(command)

    compiler_path = parts[0] if parts else 'gcc'
    compiler = os.path.basename(compiler_path)

    if compiler in ['clang++', 'c++', 'clang++-17', 'clang++-16']:
        compiler = 'g++'
    elif compiler in ['clang', 'cc', 'clang-17', 'clang-16']:
        compiler = 'gcc'

    flags = []
    i = 1

    while i < len(parts):
        part = parts[i]

        if part == source or part.endswith(source):
            i += 1
            continue

        if part == '-o' and i + 1 < len(parts):
            output = parts[i + 1]
            i += 2
            continue

        # Skip Bazel-specific flags that don't make sense outside sandbox
        if part.startswith('-fno-canonical-system-headers') or \
           part.startswith('-Wno-builtin-macro-redefined') or \
           part.startswith('-D__DATE__=') or \
           part.startswith('-D__TIMESTAMP__=') or \
           part.startswith('-D__TIME__='):
            i += 1
            continue

        if part.startswith('-'):
            if part == '-c':
                i += 1
                continue

            if part in ['-I', '-D', '-isystem', '-iquote', '-include', '-std',
                       '-march', '-mtune', '-MF', '-MD'] and i + 1 < len(parts):
                next_part = parts[i + 1]
                if not next_part.startswith('-'):
                    if part in ['-I', '-isystem', '-iquote']:
                        next_part = normalize_bazel_path(next_part, workspace_root)

                    # Skip -MF (dependency file) - we handle deps differently
                    if part == '-MF':
                        i += 2
                        continue

                    flags.append(f"{part} {next_part}")
                    i += 2
                    continue

            flags.append(part)

        i += 1

    source = normalize_bazel_path(source, workspace_root)
    if output:
        output = normalize_bazel_path(output, workspace_root)

    return compiler, source, flags, output, directory


def generate_target_label(output_file: str, workspace_root: str) -> str:
    """
    Generate a Redwood target label for an output file.

    Bazel labels like //src/main:app.o -> use similar format
    """
    rel_path = output_file

    path_parts = Path(rel_path).parts
    if len(path_parts) > 1:
        directory = '/'.join(path_parts[:-1])
        filename = path_parts[-1]
        return f"//{directory}:{filename}"
    else:
        return f"//:{rel_path}"


def find_workspace_root(compile_commands_path: str) -> str:
    """
    Find the Bazel workspace root by looking for WORKSPACE or WORKSPACE.bazel
    """
    current = os.path.dirname(os.path.abspath(compile_commands_path))

    while current != '/':
        if os.path.exists(os.path.join(current, 'WORKSPACE')) or \
           os.path.exists(os.path.join(current, 'WORKSPACE.bazel')):
            return current
        current = os.path.dirname(current)

    return os.path.dirname(os.path.abspath(compile_commands_path))


def generate_datalog(compile_commands: List[Dict], workspace_root: str) -> str:
    """Generate BUILD.datalog from Bazel's compile_commands.json"""

    lines = [
        "# Generated by bazel_to_redwood.py",
        "# Source: Bazel compile_commands.json",
        "",
    ]

    all_outputs = set()
    commands_by_output = {}

    for cmd in compile_commands:
        compiler, source, flags, output, directory = parse_bazel_compile_command(cmd, workspace_root)
        if output:
            all_outputs.add(output)
            commands_by_output[output] = (compiler, source, flags, directory)

    for output, (compiler, source, flags, directory) in commands_by_output.items():
        target_label = generate_target_label(output, workspace_root)

        if os.path.isabs(source):
            source_abs = source
        else:
            source_abs = os.path.abspath(os.path.join(workspace_root, source))

        if os.path.exists(source_abs):
            source_path = source_abs
        else:
            source_path = source

        lines.append(f'# Target: {target_label}')
        lines.append(f'target("{escape_datalog_string(target_label)}").')
        lines.append(f'kind("{escape_datalog_string(target_label)}", system_tool).')


        lines.append(f'attr("{escape_datalog_string(target_label)}", "tool", "{escape_datalog_string(compiler)}").')
        lines.append(f'attr("{escape_datalog_string(target_label)}", "-o", "{{output}}").')
        lines.append(f'attr("{escape_datalog_string(target_label)}", "-c", "{{sources}}").')

        for flag in flags:
            lines.append(f'attr("{escape_datalog_string(target_label)}", "{escape_datalog_string(flag)}", "").')

        lines.append(f'sources("{escape_datalog_string(target_label)}", "{escape_datalog_string(source_path)}").')
        lines.append(f'outputs("{escape_datalog_string(target_label)}", "{escape_datalog_string(output)}").')

        lines.append('')

    return '\n'.join(lines)


def main():
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <compile_commands.json>", file=sys.stderr)
        print("", file=sys.stderr)
        print("Generate compile_commands.json from Bazel using:", file=sys.stderr)
        print("  Option 1: https://github.com/hedronvision/bazel-compile-commands-extractor", file=sys.stderr)
        print("    bazel run @hedronv//:refresh_compile_commands", file=sys.stderr)
        print("", file=sys.stderr)
        print("  Option 2: https://github.com/grailbio/bazel-compilation-database", file=sys.stderr)
        print("    bazel build //... --experimental_action_listener=...;", file=sys.stderr)
        print("", file=sys.stderr)
        print(f"Then: python3 {sys.argv[0]} compile_commands.json > BUILD.datalog", file=sys.stderr)
        sys.exit(1)

    compile_commands_path = sys.argv[1]

    if not os.path.exists(compile_commands_path):
        print(f"Error: {compile_commands_path} not found", file=sys.stderr)
        sys.exit(1)

    workspace_root = find_workspace_root(compile_commands_path)

    with open(compile_commands_path, 'r') as f:
        compile_commands = json.load(f)

    datalog = generate_datalog(compile_commands, workspace_root)
    print(datalog)


if __name__ == '__main__':
    main()
