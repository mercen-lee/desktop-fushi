#!/usr/bin/env python3
"""Unified build entry point for Desktop Fushi.

Examples:
  scripts/build.sh desktop
  scripts/build.sh windows --arch all
  scripts/build.sh macos --arch universal
  scripts/build.sh android --variant debug --abis arm64-v8a
  scripts/build.sh android-rust --abis arm64-v8a,x86_64
"""
from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP_NAME = "Desktop Fushi"
BIN_NAME = "desktop-fushi"
MACOS_BUNDLE_IDENTIFIER = "net.mercen.desktopfushi"
DEFAULT_ANDROID_ABIS = "arm64-v8a,armeabi-v7a,x86_64"

WINDOWS_TARGETS = {
    "x64": "x86_64-pc-windows-msvc",
    "arm64": "aarch64-pc-windows-msvc",
}
MACOS_TARGETS = {
    "x64": "x86_64-apple-darwin",
    "arm64": "aarch64-apple-darwin",
}


def cargo_package_version() -> str:
    cargo_toml = ROOT / "Cargo.toml"
    in_package = False
    for raw_line in cargo_toml.read_text(encoding="utf-8").splitlines():
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
        key, sep, value = line.partition("=")
        if sep and key.strip() == "version":
            version = value.strip().strip('"').strip("'")
            if version:
                return version
    raise SystemExit(f"Cargo package version not found in {cargo_toml}")


VERSION = cargo_package_version()


def run(cmd: list[str], cwd: Path = ROOT, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def require_tool(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise SystemExit(f"Required tool not found: {name}")
    return path


def cargo_build(target: str | None, release: bool) -> None:
    require_tool("cargo")
    cmd = ["cargo", "build"]
    if release:
        cmd.append("--release")
    if target:
        require_tool("rustup")
        run(["rustup", "target", "add", target])
        cmd += ["--target", target]
    run(cmd)


def profile_dir(release: bool) -> str:
    return "release" if release else "debug"


def add_profile_flags(parser: argparse.ArgumentParser, default_release: bool = True) -> None:
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--release", dest="release", action="store_true", help="Build optimized release artifacts")
    group.add_argument("--debug", dest="release", action="store_false", help="Build debug artifacts")
    parser.set_defaults(release=default_release)


def build_desktop(args: argparse.Namespace) -> None:
    cargo_build(None, args.release)
    out_dir = ROOT / "target" / profile_dir(args.release)
    if os.name == "nt":
        src = out_dir / f"{BIN_NAME}.exe"
        dst = out_dir / f"{APP_NAME}.exe"
        if src.exists():
            shutil.copy2(src, dst)
            print(f"Built host desktop binary: {dst}")
            return
    print(f"Built host desktop binary: target/{profile_dir(args.release)}/{BIN_NAME}")


def build_windows(args: argparse.Namespace) -> None:
    arches = list(WINDOWS_TARGETS) if args.arch == "all" else [args.arch]
    for arch in arches:
        target = WINDOWS_TARGETS[arch]
        cargo_build(target, args.release)
        release_dir = ROOT / "target" / target / profile_dir(args.release)
        src = release_dir / f"{BIN_NAME}.exe"
        dst = release_dir / f"{APP_NAME}.exe"
        if src.exists():
            shutil.copy2(src, dst)
        print(f"Built Windows {arch}: {dst}")


def macos_bundle_dir(target: str, release: bool) -> Path:
    return ROOT / "target" / target / profile_dir(release) / f"{APP_NAME}.app"


def write_macos_info_plist(bundle: Path) -> None:
    (bundle / "Contents" / "Info.plist").write_text(f"""<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>ko</string>
    <key>CFBundleExecutable</key><string>{APP_NAME}</string>
    <key>CFBundleIdentifier</key><string>{MACOS_BUNDLE_IDENTIFIER}</string>
    <key>CFBundleIconFile</key><string>desktop-fushi</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleName</key><string>{APP_NAME}</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>{VERSION}</string>
    <key>CFBundleVersion</key><string>{VERSION}</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
""", encoding="utf-8")


def find_macos_codesign_identity() -> str:
    if identity := os.environ.get("DESKTOP_FUSHI_CODESIGN_IDENTITY"):
        return identity

    try:
        result = subprocess.run(
            ["security", "find-identity", "-v", "-p", "codesigning"],
            cwd=ROOT,
            check=False,
            text=True,
            capture_output=True,
        )
    except OSError:
        return "-"

    identities: list[tuple[str, str]] = []
    for line in result.stdout.splitlines():
        if ")" not in line or '"' not in line:
            continue
        prefix, _, rest = line.partition(")")
        fingerprint = rest.strip().split(" ", 1)[0]
        name = rest.partition('"')[2].rpartition('"')[0]
        if fingerprint and name:
            identities.append((fingerprint, name))

    for preferred in ["Developer ID Application", "Apple Development", "Apple Distribution"]:
        for fingerprint, name in identities:
            if preferred in name:
                return fingerprint
    return "-"


def codesign_macos_bundle(bundle: Path) -> None:
    identity = find_macos_codesign_identity()
    require_tool("codesign")
    cmd = [
        "codesign",
        "--force",
        "--deep",
        "--sign",
        identity,
        "--identifier",
        MACOS_BUNDLE_IDENTIFIER,
    ]
    if identity == "-":
        cmd += ["--requirements", f'=designated => identifier "{MACOS_BUNDLE_IDENTIFIER}"']
    cmd.append(str(bundle))
    run(cmd)


def create_macos_bundle(target_label: str, binary: Path, release: bool) -> Path:
    bundle = macos_bundle_dir(target_label, release)
    if bundle.exists():
        shutil.rmtree(bundle)
    macos_dir = bundle / "Contents" / "MacOS"
    resources_dir = bundle / "Contents" / "Resources"
    macos_dir.mkdir(parents=True)
    resources_dir.mkdir(parents=True)
    shutil.copy2(binary, macos_dir / APP_NAME)
    icon = ROOT / "assets" / "desktop-fushi.icns"
    if icon.exists():
        shutil.copy2(icon, resources_dir / "desktop-fushi.icns")
    write_macos_info_plist(bundle)
    os.chmod(macos_dir / APP_NAME, 0o755)
    codesign_macos_bundle(bundle)
    return bundle


def build_macos(args: argparse.Namespace) -> None:
    if platform.system() != "Darwin":
        raise SystemExit("macOS app bundles need Apple's SDK/linker. Run this on macOS.")
    if args.arch == "universal":
        require_tool("lipo")
        binaries: list[Path] = []
        for target in [MACOS_TARGETS["arm64"], MACOS_TARGETS["x64"]]:
            cargo_build(target, args.release)
            binaries.append(ROOT / "target" / target / profile_dir(args.release) / BIN_NAME)
        universal_dir = ROOT / "target" / "universal-apple-darwin" / profile_dir(args.release)
        universal_dir.mkdir(parents=True, exist_ok=True)
        universal_binary = universal_dir / BIN_NAME
        run(["lipo", "-create", "-output", str(universal_binary), *(str(binary) for binary in binaries)])
        bundle = create_macos_bundle("universal-apple-darwin", universal_binary, args.release)
        print(f"Created universal macOS bundle: {bundle}")
        return

    target = MACOS_TARGETS[args.arch]
    cargo_build(target, args.release)
    binary = ROOT / "target" / target / profile_dir(args.release) / BIN_NAME
    bundle = create_macos_bundle(target, binary, args.release)
    print(f"Created {bundle}")


def build_android_rust(args: argparse.Namespace) -> None:
    profile = "release" if args.release else "debug"
    cmd = [sys.executable, "scripts/build_android_rust.py", "--profile", profile, "--abis", args.abis]
    run(cmd)


def build_android(args: argparse.Namespace) -> None:
    android_dir = ROOT / "android"
    if not android_dir.exists():
        raise SystemExit("android/ project is missing")
    profile = "release" if args.variant.lower() == "release" else "debug"
    run([sys.executable, "scripts/build_android_rust.py", "--profile", profile, "--abis", args.abis])
    gradlew = android_dir / ("gradlew.bat" if os.name == "nt" else "gradlew")
    tool = str(gradlew) if gradlew.exists() else require_tool("gradle")
    task = ":app:assemble" + args.variant.capitalize()
    env = os.environ.copy()
    env["DESKTOP_FUSHI_SKIP_RUST_BUILD"] = "1"
    run([tool, task, f"-PdesktopFushiRustProfile={profile}", f"-PdesktopFushiRustAbis={args.abis}"], cwd=android_dir, env=env)
    apk_dir = android_dir / "app" / "build" / "outputs" / "apk" / args.variant.lower()
    apks = sorted(apk_dir.glob("*.apk"))
    if not apks:
        raise SystemExit(f"Expected Android APK was not produced under: {apk_dir}")
    apk = apks[0]
    print(f"Built Android APK: {apk}")


def run_desktop(args: argparse.Namespace) -> None:
    require_tool("cargo")
    cmd = ["cargo", "run"]
    if args.release:
        cmd.append("--release")
    run(cmd)


def normalize_argv(argv: list[str]) -> list[str]:
    if not argv:
        return argv
    mapping = {
        "windows-x64": ["windows", "--arch", "x64"],
        "windows-arm64": ["windows", "--arch", "arm64"],
        "windows-all": ["windows", "--arch", "all"],
        "macos-x64": ["macos", "--arch", "x64"],
        "macos-arm64": ["macos", "--arch", "arm64"],
        "macos-universal": ["macos", "--arch", "universal"],
        "android-debug": ["android", "--variant", "debug"],
        "android-release": ["android", "--variant", "release"],
    }
    head = argv[0]
    if head in mapping:
        return mapping[head] + argv[1:]
    return argv


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Desktop Fushi unified build script")
    sub = p.add_subparsers(dest="command")

    check = sub.add_parser("check", help="Run cargo check for the host desktop target")
    check.set_defaults(func=lambda _args: run(["cargo", "check"]))

    host = sub.add_parser("desktop", help="Build the host desktop binary")
    add_profile_flags(host)
    host.set_defaults(func=build_desktop)

    win = sub.add_parser("windows", help="Build Windows targets")
    win.add_argument("--arch", choices=["all", "x64", "arm64"], default="all")
    add_profile_flags(win)
    win.set_defaults(func=build_windows)

    mac = sub.add_parser("macos", help="Build a macOS .app bundle")
    mac.add_argument("--arch", choices=["arm64", "x64", "universal"], default="arm64")
    add_profile_flags(mac)
    mac.set_defaults(func=build_macos)

    android = sub.add_parser("android", help="Build the Android wgpu overlay APK")
    android.add_argument("--variant", choices=["debug", "release"], default="debug")
    android.add_argument("--abis", "--abi", dest="abis", default=DEFAULT_ANDROID_ABIS)
    android.set_defaults(func=build_android)

    android_rust = sub.add_parser("android-rust", help="Build only Android Rust/wgpu shared libraries")
    android_rust.add_argument("--abis", "--abi", dest="abis", default=DEFAULT_ANDROID_ABIS)
    add_profile_flags(android_rust, default_release=False)
    android_rust.set_defaults(func=build_android_rust)

    runp = sub.add_parser("run", help="Run the desktop app")
    add_profile_flags(runp)
    runp.set_defaults(func=run_desktop)
    return p


def main(argv: list[str] | None = None) -> None:
    p = parser()
    args = p.parse_args(normalize_argv(sys.argv[1:] if argv is None else argv))
    if not hasattr(args, "func"):
        args.release = True
        build_desktop(args)
        return
    args.func(args)


if __name__ == "__main__":
    main()
