#!/usr/bin/env python3
"""Stream Cargo test output as a short progress list and a final verdict."""

import re
import subprocess
import sys

TEST_LINE = re.compile(r"^test (\S+) \.\.\. (\S+)\s*$")
RUNNING_N = re.compile(r"^running (\d+) tests?$")
RESULT = re.compile(
    r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out"
)
RUNNING_TARGET = re.compile(r"^     Running (.+?) \((.+)\)\s*$")
DOC_TARGET = re.compile(r"^   Doc-tests (\S+)\s*$")


def color(code, text):
    if not sys.stdout.isatty():
        return text
    return f"\033[{code}m{text}\033[0m"


def suite_label(kind, path):
    if kind == "doc":
        return f"{path} docs"
    crate = "tests"
    match = re.search(r"/deps/([\w-]+)-[0-9a-f]+$", path or "")
    if match:
        crate = match.group(1).replace("_", "-")
    if kind.startswith("tests/"):
        name = kind.removeprefix("tests/").removesuffix(".rs")
        if crate == name:
            return f"yeelight {name}"
        return f"{crate} {name}"
    return crate


def main():
    if len(sys.argv) < 2:
        print("usage: test-report.py COMMAND [ARGS...]", file=sys.stderr)
        return 2

    passed = failed = ignored = 0
    empty = []
    suite = None
    skip_result = False
    show_details = False
    compile_lines = []

    process = subprocess.Popen(
        sys.argv[1:],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    assert process.stdout is not None

    for raw in process.stdout:
        line = raw.rstrip("\n")

        if show_details:
            if line.startswith("     Running ") or line.startswith("   Doc-tests "):
                show_details = False
            else:
                if line.strip() and not line.startswith("error: test failed"):
                    print(color("2", line))
                continue

        target = RUNNING_TARGET.match(line)
        docs = DOC_TARGET.match(line)
        if target or docs:
            if target:
                suite = suite_label(target.group(1), target.group(2) or "")
            else:
                suite = suite_label("doc", docs.group(1))
            continue

        count = RUNNING_N.match(line)
        if count:
            if count.group(1) == "0":
                empty.append(suite or "target")
                suite = None
                skip_result = True
            else:
                print()
                print(color("1", f"  {suite or 'tests'}"))
            continue

        test = TEST_LINE.match(line)
        if test:
            name, status = test.group(1), test.group(2)
            if status == "ok":
                print(color("32", f"    ✓ {name}"))
            elif status == "ignored":
                print(color("33", f"    – {name}"))
            else:
                print(color("31", f"    ✗ {name}"))
            continue

        result = RESULT.match(line)
        if result:
            if not skip_result:
                passed += int(result.group(1))
                failed += int(result.group(2))
                ignored += int(result.group(3))
            skip_result = False
            continue

        if line.startswith("failures:"):
            show_details = True
            print()
            print(color("31", line))
            continue

        if line.startswith("   Compiling ") or line.startswith("    Finished "):
            compile_lines.append(line.strip())
            continue

        if line.startswith("error") or line.startswith("warning:"):
            print(line)
            continue

    status = process.wait()

    if compile_lines and passed + failed + ignored == 0 and status != 0:
        for line in compile_lines:
            print(color("2", line))

    print()
    print(color("2", "  " + "─" * 42))
    quiet = ""
    if empty:
        quiet = color("2", f"  {len(empty)} targets had no tests.")
    if status != 0 and passed + failed == 0:
        print(color("31", "  Tests failed before any result was reported."))
    elif failed:
        print(color("31;1", f"  {failed} failed, {passed} passed."))
    else:
        print(color("32;1", f"  All {passed} tests passed."))
    if ignored:
        print(color("33", f"  {ignored} ignored."))
    if quiet:
        print(quiet)
    return status


if __name__ == "__main__":
    sys.exit(main())
