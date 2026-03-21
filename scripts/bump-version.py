#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# ///

import json
import re
import sys
from pathlib import Path

def main():
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <version>")
        print("Example: ./scripts/bump-version.py 0.1.0")
        sys.exit(1)

    version = sys.argv[1]
    root = Path(__file__).resolve().parent.parent

    # Cargo.toml
    cargo = root / "Cargo.toml"
    text = cargo.read_text()
    text = re.sub(r'^version = ".*"', f'version = "{version}"', text, count=1, flags=re.MULTILINE)
    cargo.write_text(text)

    # npm packages
    npm_pkgs = [
        root / "npm/heap-snapshot/package.json",
        root / "npm/darwin-arm64/package.json",
        root / "npm/linux-x64/package.json",
        root / "npm/linux-arm64/package.json",
        root / "npm/win32-x64/package.json",
        root / "npm/win32-arm64/package.json",
    ]

    for pkg_path in npm_pkgs:
        pkg = json.loads(pkg_path.read_text())
        pkg["version"] = version
        if "optionalDependencies" in pkg:
            for dep in pkg["optionalDependencies"]:
                pkg["optionalDependencies"][dep] = version
        pkg_path.write_text(json.dumps(pkg, indent=2) + "\n")

    print(f"Bumped version to {version}")

if __name__ == "__main__":
    main()
