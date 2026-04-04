#!/usr/bin/env -S uv run
"""Run all formatters: cargo fmt and prettier."""

import subprocess
import sys
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.path.join(ROOT, "web")


def run(cmd: list[str], cwd: str = ROOT) -> bool:
    result = subprocess.run(cmd, cwd=cwd)
    return result.returncode == 0


def main() -> int:
    ok = True
    ok = run(["cargo", "fmt"]) and ok
    ok = run(["npm", "run", "fmt"], cwd=WEB) and ok
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
