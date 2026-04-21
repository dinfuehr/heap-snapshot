#!/usr/bin/env -S uv run
"""Run all tests and checks: Rust tests, formatting, TypeScript, and Playwright e2e."""

import subprocess
import sys
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.path.join(ROOT, "web")

def run(cmd: list[str], cwd: str = ROOT) -> bool:
    print(f"\n{'=' * 60}")
    print(f"  {' '.join(cmd)}")
    print(f"  (in {cwd})")
    print(f"{'=' * 60}\n")
    result = subprocess.run(cmd, cwd=cwd)
    if result.returncode != 0:
        print(f"\nFAILED: {' '.join(cmd)}")
    return result.returncode == 0

def main() -> int:
    ok = True

    # Rust
    ok = run(["cargo", "fmt", "--check"]) and ok
    ok = run(["cargo", "test"]) and ok

    # Web: build WASM first (needed for typecheck and e2e). Skip the rest if
    # it fails, since typecheck and e2e would only produce noise on stale WASM.
    wasm_ok = run(["npm", "run", "build:wasm"], cwd=WEB)
    ok = wasm_ok and ok

    if wasm_ok:
        # Web: checks
        ok = run(["npm", "run", "typecheck"], cwd=WEB) and ok
        ok = run(["npx", "prettier", "--check", "src/**/*.{ts,tsx}", "e2e/**/*.ts", "vite.config.ts"], cwd=WEB) and ok

        # Web: e2e tests
        ok = run(["npm", "run", "test:e2e"], cwd=WEB) and ok
    else:
        print("\nSkipping typecheck, prettier, and e2e: WASM build failed.")

    print(f"\n{'=' * 60}")
    if ok:
        print("  ALL TESTS PASSED")
    else:
        print("  SOME TESTS FAILED")
    print(f"{'=' * 60}\n")

    return 0 if ok else 1

if __name__ == "__main__":
    sys.exit(main())
