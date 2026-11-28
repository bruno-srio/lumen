#!/usr/bin/env python3
"""Reject an unsafe LUMEN_WORK and print its resolved path."""

import os
import sys


def inside(child, parent):
    child = os.path.normpath(child)
    parent = os.path.normpath(parent)
    return child == parent or child.startswith(parent + os.sep)


def main():
    if len(sys.argv) != 3:
        print("usage: require_work.py LUMEN_WORK REPO", file=sys.stderr)
        return 2
    given = os.path.abspath(sys.argv[1])
    repo = os.path.realpath(sys.argv[2])
    resolved = os.path.realpath(given)
    if resolved != os.path.normpath(given):
        print(
            f"error: LUMEN_WORK resolves through a symlink ({given} -> {resolved})",
            file=sys.stderr,
        )
        return 2
    home = os.path.realpath(os.path.expanduser("~"))
    protected = [
        os.path.join(home, ".config"),
        os.path.join(home, ".var"),
        os.path.join(home, ".local"),
        os.path.join(home, "Code", "repos"),
        repo,
    ]
    for path in protected:
        path = os.path.realpath(path)
        if inside(resolved, path) or (inside(path, resolved) and resolved != path):
            print(
                f"error: LUMEN_WORK {resolved} is equal to, inside, or an ancestor of {path}",
                file=sys.stderr,
            )
            return 2
    print(resolved)
    return 0


if __name__ == "__main__":
    sys.exit(main())
