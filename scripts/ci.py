#!/usr/bin/env python3
"""Continuous-integration entry points for Desktop Fushi.

The GitHub workflows install the platform toolchains; this file keeps the
repository-specific checks and build invocations in one locally runnable place.

Examples:
  python scripts/ci.py quality
  python scripts/ci.py desktop --platform windows --arch x64
  python scripts/ci.py android --abis arm64-v8a
  python scripts/ci.py web
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WEB_DIR = ROOT / "web"


def run(command: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(command))
    subprocess.run(command, cwd=cwd, env=env, check=True)


def cargo_package_version() -> str:
    in_package = False
    for raw_line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line == "[package]":
            in_package = True
            continue
        if line.startswith("[") and line.endswith("]"):
            in_package = False
            continue
        if not in_package:
            continue
        key, separator, value = line.partition("=")
        if separator and key.strip() == "version":
            version = value.strip().strip('"').strip("'")
            if version:
                return version
    raise SystemExit("Cargo package version not found")


def quality(_: argparse.Namespace) -> None:
    run([sys.executable, "-m", "compileall", "-q", "scripts"])
    run(["cargo", "fmt", "--all", "--", "--check"])
    run(["cargo", "check", "--locked", "--all-targets"])
    # SVG reference geometry intentionally preserves literal source coordinates.
    run(["cargo", "clippy", "--locked", "--all-targets", "--", "-A", "clippy::approx_constant"])
    run(["cargo", "test", "--locked", "--all-targets"])


def desktop(args: argparse.Namespace) -> None:
    run(
        [
            sys.executable,
            "scripts/build.py",
            args.platform,
            "--arch",
            args.arch,
            "--release",
        ]
    )


def android(args: argparse.Namespace) -> None:
    run(
        [
            sys.executable,
            "scripts/build.py",
            "android",
            "--variant",
            "debug",
            "--abis",
            args.abis,
        ]
    )


def web(_: argparse.Namespace) -> None:
    env = os.environ.copy()
    env["PUBLIC_DESKTOP_FUSHI_VERSION"] = cargo_package_version()
    run(["npm", "ci"], cwd=WEB_DIR, env=env)
    run(["npm", "run", "build"], cwd=WEB_DIR, env=env)


def parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Desktop Fushi CI commands")
    commands = parser.add_subparsers(dest="command", required=True)

    quality_parser = commands.add_parser("quality", help="Run formatting, checks, and tests")
    quality_parser.set_defaults(func=quality)

    desktop_parser = commands.add_parser("desktop", help="Build a desktop release as a smoke test")
    desktop_parser.add_argument("--platform", choices=["windows", "macos"], required=True)
    desktop_parser.add_argument("--arch", choices=["x64", "arm64"], required=True)
    desktop_parser.set_defaults(func=desktop)

    android_parser = commands.add_parser("android", help="Build a debug Android APK")
    android_parser.add_argument("--abis", default="arm64-v8a")
    android_parser.set_defaults(func=android)

    web_parser = commands.add_parser("web", help="Build the WebAssembly and Astro site")
    web_parser.set_defaults(func=web)
    return parser


def main() -> None:
    args = parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
