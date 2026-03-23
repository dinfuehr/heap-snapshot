#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["psutil"]
# ///

import os
import sys
import time
import psutil


def main():
    max_rss: dict[int, float] = {}
    # PIDs we've seen that are no longer alive — keep to show "exited" line.
    exited: dict[int, float] = {}
    prev_rows = 0

    while True:
        alive_pids: set[int] = set()
        lines: list[str] = []

        for proc in psutil.process_iter(["pid", "name", "cmdline", "memory_info"]):
            try:
                info = proc.info
                name = info["name"] or ""
                cmdline = " ".join(info["cmdline"] or [])
                if "heap-snapshot" not in name and "heap-snapshot" not in cmdline:
                    continue
                pid = info["pid"]
                rss_mb = info["memory_info"].rss / (1024 * 1024)
                max_rss[pid] = max(max_rss.get(pid, 0), rss_mb)
                alive_pids.add(pid)
                exited.pop(pid, None)
                lines.append(
                    f"PID {pid:>7}  RSS {rss_mb:>8.1f} MB  max {max_rss[pid]:>8.1f} MB  {cmdline[:100]}"
                )
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                continue

        # Detect newly exited processes.
        for pid in list(max_rss):
            if pid not in alive_pids and pid not in exited:
                exited[pid] = max_rss[pid]

        for pid, peak in exited.items():
            lines.append(f"PID {pid:>7}  [exited]           max {peak:>8.1f} MB")

        if not lines:
            lines.append("(no heap-snapshot processes found)")

        # Move cursor up to overwrite previous output.
        if prev_rows > 0:
            sys.stdout.write(f"\033[{prev_rows}A\033[J")

        output = "\n".join(lines)
        sys.stdout.write(output + "\n")
        sys.stdout.flush()

        # Count physical terminal rows consumed (lines that wrap use >1 row).
        cols = os.get_terminal_size(sys.stdout.fileno()).columns
        prev_rows = sum((max(len(line), 1) - 1) // cols + 1 for line in lines)

        time.sleep(1)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
