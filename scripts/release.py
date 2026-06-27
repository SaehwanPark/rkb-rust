#!/usr/bin/env python3
"""Reusable release helpers for rkb-rust.

The helpers intentionally default to dry-run checks. Registry publication and
tap updates remain controlled by GitHub Actions secrets and explicit tags.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONFIG_PATH = ROOT / "release.toml"


def load_config() -> dict:
    with CONFIG_PATH.open("rb") as handle:
        return tomllib.load(handle)


def run(command: str, *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    print(f"+ {command}", flush=True)
    completed = subprocess.run(
        command,
        cwd=ROOT,
        shell=True,
        text=True,
        capture_output=capture,
    )
    if completed.returncode != 0:
        if capture and completed.stdout:
            print(completed.stdout, end="")
        if capture and completed.stderr:
            print(completed.stderr, end="", file=sys.stderr)
        raise subprocess.CalledProcessError(
            completed.returncode,
            command,
            output=completed.stdout,
            stderr=completed.stderr,
        )
    return completed


def require_python_version() -> None:
    if sys.version_info < (3, 11):
        raise SystemExit("release scripts require Python 3.11 or newer for tomllib")


def release_check(config: dict) -> None:
    for command in config["checks"]["commands"]:
        run(command)


def package_check(config: dict) -> None:
    listing = run("cargo package --locked --allow-dirty --list", capture=True).stdout.splitlines()
    denied = []
    deny_prefixes = tuple(config["package"]["deny_prefixes"])
    for item in listing:
        path = item.strip()
        if path.startswith(deny_prefixes):
            denied.append(path)
    if denied:
        print("cargo package includes denied paths:", file=sys.stderr)
        for path in denied:
            print(f"  {path}", file=sys.stderr)
        raise SystemExit(1)

    for path in listing:
        print(path)

    run("cargo publish --dry-run --locked --allow-dirty")


def dist_plan(config: dict, *, build: bool) -> None:
    if shutil.which("dist") is None and shutil.which("cargo-dist") is None:
        raise SystemExit(
            "dist/cargo-dist is not installed. Install it with `cargo install cargo-dist`."
        )

    command = "dist build --artifacts=global" if build else "dist plan"
    run(command)

    project = config["project"]
    homebrew = config["homebrew"]
    print()
    print("Release configuration:")
    print(f"  tag pattern: {project['tag_prefix']}<version>")
    print(f"  crate: {project['package']}")
    print(f"  binary: {project['binary']}")
    print(f"  homebrew tap: {homebrew['tap']}")
    print(f"  homebrew formula: {homebrew['formula']}")


def main() -> None:
    require_python_version()
    config = load_config()

    parser = argparse.ArgumentParser(description="Run reusable release helpers.")
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("check", help="Run formatting, linting, tests, and docs.")
    subcommands.add_parser("package", help="Inspect package contents and dry-run publish.")
    plan_parser = subcommands.add_parser("plan", help="Run dist release planning.")
    plan_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Accepted for clarity; planning is the default non-publishing mode.",
    )
    plan_parser.add_argument(
        "--build",
        action="store_true",
        help="Run a local dist build instead of only planning.",
    )

    args = parser.parse_args()

    if args.command == "check":
        release_check(config)
    elif args.command == "package":
        package_check(config)
    elif args.command == "plan":
        dist_plan(config, build=args.build)
    else:
        parser.error(f"unknown command: {args.command}")


if __name__ == "__main__":
    main()
