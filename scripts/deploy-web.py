#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.10"
# ///
import argparse
import subprocess
import shutil
import sys
from pathlib import Path

repo_dir = Path(__file__).resolve().parent.parent
build_repo = repo_dir / ".." / "heap-snapshot-web-build"
web_dir = repo_dir / "web"
dist_dir = web_dir / "dist"


def run(cmd, **kwargs):
    return subprocess.run(cmd, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--no-check", action="store_true", help="Skip uncommitted changes check")
    parser.add_argument("--allow-empty", action="store_true", help="Commit even if there are no changes")
    args = parser.parse_args()

    # Check for uncommitted changes
    if not args.no_check:
        result = subprocess.run(
            ["git", "status", "--porcelain"], cwd=repo_dir, capture_output=True, text=True
        )
        if result.stdout.strip():
            print("Error: there are uncommitted changes in heap-snapshot", file=sys.stderr)
            sys.exit(1)

    commit_hash = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_dir,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()

    # Build
    run(["npm", "run", "build"], cwd=web_dir)

    # Copy build output to deploy repo
    for item in build_repo.iterdir():
        if item.name == ".git":
            continue
        if item.is_dir():
            shutil.rmtree(item)
        else:
            item.unlink()

    for item in dist_dir.iterdir():
        dest = build_repo / item.name
        if item.is_dir():
            shutil.copytree(item, dest)
        else:
            shutil.copy2(item, dest)

    # Commit
    run(["git", "add", "-A"], cwd=build_repo)
    status = subprocess.run(
        ["git", "diff", "--cached", "--quiet"], cwd=build_repo
    )
    if status.returncode == 0 and not args.allow_empty:
        print("No changes to deploy")
        return
    allow_empty = ["--allow-empty"] if status.returncode == 0 else []
    run(["git", "commit", *allow_empty, "-m", f"Deploy heap-snapshot@{commit_hash}"], cwd=build_repo)

    print(f"Deployed heap-snapshot@{commit_hash}")


if __name__ == "__main__":
    main()
