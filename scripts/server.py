#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Serve the heap-snapshot web UI.

Usage:
    uv run scripts/server.py [--port PORT]

Builds the WASM package (if needed) and starts the Vite dev server.
"""

import argparse
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WEB = ROOT / "web"


def run(cmd: list[str], **kwargs) -> None:
    print(f"$ {' '.join(cmd)}")
    subprocess.check_call(cmd, **kwargs)


def main() -> None:
    parser = argparse.ArgumentParser(description="Serve the heap-snapshot web UI")
    parser.add_argument("--port", type=int, default=5173, help="Port (default: 5173)")
    args = parser.parse_args()

    # Ensure npm dependencies are installed
    if not (WEB / "node_modules").exists():
        run(["npm", "--prefix", str(WEB), "install"])

    # Build WASM if needed
    if not (WEB / "wasm-pkg").exists():
        run(["npm", "--prefix", str(WEB), "run", "build:wasm"])

    # Start Vite dev server
    try:
        run(["npx", "--prefix", str(WEB), "vite", "--port", str(args.port)], cwd=str(WEB))
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
