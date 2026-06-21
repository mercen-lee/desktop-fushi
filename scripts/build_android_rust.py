#!/usr/bin/env python3
"""Build the shared Rust/wgpu library for Android ABIs.

This script is intentionally Gradle-independent so the same native build can be
called from scripts/build.sh android, Android Studio/Gradle, or CI.
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
ANDROID_DIR = ROOT / "android"
JNILIBS_DIR = ANDROID_DIR / "app" / "src" / "main" / "jniLibs"
LIB_NAME = "libdesktop_fushi.so"

ABIS = {
    "arm64-v8a": ("aarch64-linux-android", "aarch64-linux-android"),
    "armeabi-v7a": ("armv7-linux-androideabi", "armv7a-linux-androideabi"),
    "x86_64": ("x86_64-linux-android", "x86_64-linux-android"),
}


def run(cmd: list[str], env: dict[str, str] | None = None) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, cwd=ROOT, env=env, check=True)


def require_tool(name: str) -> str:
    path = shutil.which(name)
    if not path:
        raise SystemExit(f"Required tool not found: {name}")
    return path


def parse_local_properties() -> dict[str, str]:
    props: dict[str, str] = {}
    local = ANDROID_DIR / "local.properties"
    if not local.exists():
        return props
    for line in local.read_text(encoding="utf-8", errors="ignore").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        props[key.strip()] = value.strip().replace("\\\\", "\\")
    return props


def newest_ndk_from_sdk(sdk: Path) -> Path | None:
    ndk_dir = sdk / "ndk"
    if not ndk_dir.exists():
        return None
    versions = [p for p in ndk_dir.iterdir() if p.is_dir()]
    if not versions:
        return None
    return sorted(versions, key=lambda p: p.name)[-1]


def find_ndk() -> Path:
    for var in ("ANDROID_NDK_HOME", "ANDROID_NDK_ROOT", "NDK_HOME"):
        value = os.environ.get(var)
        if value and Path(value).exists():
            return Path(value)
    props = parse_local_properties()
    if props.get("ndk.dir") and Path(props["ndk.dir"]).exists():
        return Path(props["ndk.dir"])
    for var in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        value = os.environ.get(var) or props.get("sdk.dir")
        if value:
            ndk = newest_ndk_from_sdk(Path(value))
            if ndk:
                return ndk
    raise SystemExit(
        "Android NDK not found. Set ANDROID_NDK_HOME/ANDROID_NDK_ROOT, "
        "or add ndk.dir to android/local.properties."
    )


def host_tag() -> str:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Darwin":
        return "darwin-x86_64"
    if system == "Linux":
        return "linux-x86_64"
    if system == "Windows":
        return "windows-x86_64"
    raise SystemExit(f"Unsupported host OS for Android NDK: {system}")


def toolchain_bin(ndk: Path) -> Path:
    path = ndk / "toolchains" / "llvm" / "prebuilt" / host_tag() / "bin"
    if not path.exists():
        raise SystemExit(f"Android NDK LLVM toolchain not found: {path}")
    return path


def exe(name: str) -> str:
    return name + (".cmd" if os.name == "nt" else "")


def cargo_target_env_name(target: str) -> str:
    return "CARGO_TARGET_" + target.upper().replace("-", "_") + "_LINKER"


def cc_env_name(target: str) -> str:
    return "CC_" + target.replace("-", "_")


def append_rustflags(env: dict[str, str], flags: list[str]) -> None:
    current = env.get("RUSTFLAGS", "").strip()
    suffix = " ".join(flags)
    if suffix not in current:
        env["RUSTFLAGS"] = f"{current} {suffix}".strip()


def build_one(abi: str, cargo_target: str, clang_triple: str, profile: str, api: int, ndk_bin: Path) -> None:
    require_tool("cargo")
    require_tool("rustup")
    run(["rustup", "target", "add", cargo_target])

    clang = ndk_bin / exe(f"{clang_triple}{api}-clang")
    if not clang.exists():
        raise SystemExit(f"Android clang not found: {clang}")

    env = os.environ.copy()
    env[cargo_target_env_name(cargo_target)] = str(clang)
    env[cc_env_name(cargo_target)] = str(clang)
    append_rustflags(env, ["-C", "link-arg=-Wl,-z,max-page-size=16384"])

    cmd = ["cargo", "build", "--lib", "--target", cargo_target]
    if profile == "release":
        cmd.append("--release")
    run(cmd, env=env)

    source = ROOT / "target" / cargo_target / profile / LIB_NAME
    if not source.exists():
        raise SystemExit(f"Expected Android Rust library was not produced: {source}")
    out_dir = JNILIBS_DIR / abi
    out_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, out_dir / LIB_NAME)
    print(f"Copied {abi}: {out_dir / LIB_NAME}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Build Desktop Fushi Rust/wgpu Android libraries")
    parser.add_argument("--profile", choices=["debug", "release"], default="debug")
    parser.add_argument("--abis", default="arm64-v8a,armeabi-v7a,x86_64")
    parser.add_argument("--api", type=int, default=26)
    args = parser.parse_args()

    requested = [abi.strip() for abi in args.abis.split(",") if abi.strip()]
    unknown = [abi for abi in requested if abi not in ABIS]
    if unknown:
        raise SystemExit(f"Unsupported ABI(s): {', '.join(unknown)}")

    ndk = find_ndk()
    ndk_bin = toolchain_bin(ndk)
    print(f"Using Android NDK: {ndk}")
    for abi in requested:
        cargo_target, clang_triple = ABIS[abi]
        build_one(abi, cargo_target, clang_triple, args.profile, args.api, ndk_bin)


if __name__ == "__main__":
    main()
