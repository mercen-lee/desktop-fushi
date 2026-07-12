#!/usr/bin/env python3
"""Continuous-delivery operations for Desktop Fushi.

GitHub Actions provides runners, credentials, caches, and artifact transport.
This command owns repository-specific release policy, packaging, signing, and
deployment steps so those details are not duplicated across workflow YAML.
"""
from __future__ import annotations

import argparse
import base64
import binascii
import os
import re
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WEB_DIR = ROOT / "web"
TAG_PATTERN = re.compile(r"v[0-9]+\.[0-9]+\.[0-9]+$")
VERSION_PATTERN = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+$")
WINDOWS_TARGETS = {
    "x64": "x86_64-pc-windows-msvc",
    "arm64": "aarch64-pc-windows-msvc",
}


def fail(message: str) -> None:
    raise SystemExit(message)


def run(
    command: list[str],
    *,
    cwd: Path = ROOT,
    env: dict[str, str] | None = None,
    redact: tuple[str, ...] = (),
) -> None:
    display = ["***" if any(secret and secret in value for secret in redact) else value for value in command]
    print("+", " ".join(display))
    subprocess.run(command, cwd=cwd, env=env, check=True)


def require_environment(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        fail(f"Required environment variable {name} is not configured")
    return value


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
    fail("Cargo package version not found")


def validate_version(version: str) -> str:
    if not VERSION_PATTERN.fullmatch(version):
        fail(f"Version must match X.Y.Z: {version}")
    return version


def write_github_output(path: Path | None, **values: str) -> None:
    if path is None:
        for key, value in values.items():
            print(f"{key}={value}")
        return
    with path.open("a", encoding="utf-8") as output:
        for key, value in values.items():
            output.write(f"{key}={value}\n")


def metadata(args: argparse.Namespace) -> None:
    tag = args.tag
    if not TAG_PATTERN.fullmatch(tag):
        fail(f"Release tag must match vX.Y.Z: {tag}")
    verified = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"refs/tags/{tag}^{{commit}}"],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if verified.returncode != 0:
        fail(f"Release tag does not exist locally: {tag}")
    version = cargo_package_version()
    if tag != f"v{version}":
        fail(f"Release tag {tag} does not match Cargo package version v{version}")
    write_github_output(args.github_output, tag=tag, version=version)


def package_windows(args: argparse.Namespace) -> None:
    version = validate_version(args.version)
    target = args.target or WINDOWS_TARGETS[args.arch]
    source = ROOT / "target" / target / "release" / "Desktop Fushi.exe"
    if not source.is_file():
        fail(f"Windows executable was not produced: {source}")
    destination = args.dist_dir / f"desktop-fushi-v{version}-windows-{args.arch}.zip"
    destination.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(source, arcname="Desktop Fushi.exe")
    print(f"Packaged Windows {args.arch}: {destination}")


def package_macos(args: argparse.Namespace) -> None:
    version = validate_version(args.version)
    ditto = shutil.which("ditto")
    if ditto is None:
        fail("macOS packaging requires ditto")
    args.dist_dir.mkdir(parents=True, exist_ok=True)
    bundles = {
        "arm64": ROOT / "target" / "aarch64-apple-darwin" / "release" / "Desktop Fushi.app",
        "universal": ROOT / "target" / "universal-apple-darwin" / "release" / "Desktop Fushi.app",
    }
    for architecture, bundle in bundles.items():
        if not bundle.is_dir():
            fail(f"macOS bundle was not produced: {bundle}")
        destination = args.dist_dir / f"desktop-fushi-v{version}-macos-{architecture}.zip"
        run([ditto, "-c", "-k", "--norsrc", "--keepParent", str(bundle), str(destination)])


def prepare_android_keystore(args: argparse.Namespace) -> None:
    encoded = require_environment("ANDROID_KEYSTORE_BASE64")
    require_environment("ANDROID_KEYSTORE_PASSWORD")
    require_environment("ANDROID_KEY_ALIAS")
    require_environment("ANDROID_KEY_PASSWORD")
    try:
        keystore = base64.b64decode("".join(encoded.split()), validate=True)
    except (binascii.Error, ValueError) as error:
        fail(f"ANDROID_KEYSTORE_BASE64 is not valid Base64: {error}")
    if not keystore:
        fail("ANDROID_KEYSTORE_BASE64 decoded to an empty keystore")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(keystore)
    args.output.chmod(0o600)
    write_github_output(args.github_env, ANDROID_KEYSTORE_PATH=str(args.output))


def sign_android(args: argparse.Namespace) -> None:
    version = validate_version(args.version)
    if not args.keystore.is_file():
        fail(f"Android signing keystore does not exist: {args.keystore}")
    alias = require_environment("ANDROID_KEY_ALIAS")
    require_environment("ANDROID_KEYSTORE_PASSWORD")
    require_environment("ANDROID_KEY_PASSWORD")
    build_tools = args.sdk_root / "build-tools" / args.build_tools_version
    zipalign = build_tools / "zipalign"
    apksigner = build_tools / "apksigner"
    aapt = build_tools / "aapt"
    for tool in (zipalign, apksigner, aapt):
        if not tool.is_file():
            fail(f"Android build tool not found: {tool}")

    release_dir = ROOT / "android" / "app" / "build" / "outputs" / "apk" / "release"
    apks = sorted(release_dir.glob("*.apk"))
    if len(apks) != 1:
        fail(f"Expected one Gradle release APK, found {len(apks)} under {release_dir}")
    args.dist_dir.mkdir(parents=True, exist_ok=True)
    asset = args.dist_dir / f"pocket-fushi-v{version}-android-universal.apk"
    temporary_dir = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))
    aligned_apk = temporary_dir / "pocket-fushi-aligned.apk"
    run([str(zipalign), "-f", "-p", "4", str(apks[0]), str(aligned_apk)])
    run(
        [
            str(apksigner),
            "sign",
            "--ks",
            str(args.keystore),
            "--ks-key-alias",
            alias,
            "--ks-pass",
            "env:ANDROID_KEYSTORE_PASSWORD",
            "--key-pass",
            "env:ANDROID_KEY_PASSWORD",
            "--out",
            str(asset),
            str(aligned_apk),
        ]
    )
    run([str(apksigner), "verify", "--verbose", str(asset)])
    badging = subprocess.run(
        [str(aapt), "dump", "badging", str(asset)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    if "application-label:'Pocket Fushi'" not in badging.stdout.splitlines():
        fail("Signed Android APK is not labeled Pocket Fushi")


def expected_assets(version: str) -> set[str]:
    return {
        f"desktop-fushi-v{version}-windows-x64.zip",
        f"desktop-fushi-v{version}-windows-arm64.zip",
        f"desktop-fushi-v{version}-macos-arm64.zip",
        f"desktop-fushi-v{version}-macos-universal.zip",
        f"pocket-fushi-v{version}-android-universal.apk",
    }


def verify_assets(args: argparse.Namespace) -> None:
    version = validate_version(args.version)
    assets = {path.name for path in args.dist_dir.iterdir() if path.is_file()}
    expected = expected_assets(version)
    if assets != expected:
        fail(f"Release assets differ from expected bundles: {sorted(assets)}")

    for archive in args.dist_dir.glob("*windows-*.zip"):
        with zipfile.ZipFile(archive) as zip_file:
            names = [name for name in zip_file.namelist() if name and not name.endswith("/")]
        if names != ["Desktop Fushi.exe"]:
            fail(f"{archive} must contain only Desktop Fushi.exe")

    for archive in args.dist_dir.glob("*macos-*.zip"):
        with zipfile.ZipFile(archive) as zip_file:
            roots = {name.split("/", 1)[0] for name in zip_file.namelist() if name}
        if roots != {"Desktop Fushi.app"}:
            fail(f"{archive} must contain only Desktop Fushi.app")

    android_asset = args.dist_dir / f"pocket-fushi-v{version}-android-universal.apk"
    with zipfile.ZipFile(android_asset) as apk:
        invalid_member = apk.testzip()
    if invalid_member is not None:
        fail(f"{android_asset} contains an invalid member: {invalid_member}")


def write_release_notes(args: argparse.Namespace) -> None:
    version = validate_version(args.version)
    args.output.write_text(
        "\n".join(
            [
                f"Automated build for Cargo package version {version}.",
                "",
                "Desktop Fushi",
                f"- desktop-fushi-v{version}-windows-x64.zip",
                f"- desktop-fushi-v{version}-windows-arm64.zip",
                f"- desktop-fushi-v{version}-macos-arm64.zip",
                f"- desktop-fushi-v{version}-macos-universal.zip",
                "",
                "Pocket Fushi (Android universal APK)",
                f"- pocket-fushi-v{version}-android-universal.apk",
                "",
            ]
        ),
        encoding="utf-8",
    )


def publish_github(args: argparse.Namespace) -> None:
    version = validate_version(args.version)
    if not TAG_PATTERN.fullmatch(args.tag) or args.tag != f"v{version}":
        fail(f"Release tag must be v{version}: {args.tag}")
    repo = args.repo or os.environ.get("GH_REPO") or os.environ.get("GITHUB_REPOSITORY")
    if not repo:
        fail("Set GH_REPO or GITHUB_REPOSITORY before publishing a release")
    assets = [str(path) for path in sorted(args.dist_dir.iterdir()) if path.is_file()]
    title = f"Fushi v{version}"
    view = subprocess.run(
        ["gh", "release", "view", args.tag, "--repo", repo],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if view.returncode == 0:
        run(["gh", "release", "edit", args.tag, "--repo", repo, "--title", title, "--notes-file", str(args.notes)])
        run(["gh", "release", "upload", args.tag, "--repo", repo, *assets, "--clobber"])
        return
    run(
        [
            "gh",
            "release",
            "create",
            args.tag,
            "--repo",
            repo,
            "--verify-tag",
            "--title",
            title,
            "--notes-file",
            str(args.notes),
            *assets,
        ]
    )


def resolve_web_version(args: argparse.Namespace) -> None:
    requested = (args.requested_version or os.environ.get("REQUESTED_VERSION", "")).removeprefix("v")
    if requested:
        version = validate_version(requested)
    else:
        repo = args.repo or os.environ.get("GITHUB_REPOSITORY")
        if not repo:
            fail("Set GITHUB_REPOSITORY before resolving the latest release")
        release = subprocess.run(
            ["gh", "release", "view", "--repo", repo, "--json", "tagName", "--jq", ".tagName"],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=True,
        )
        tag = release.stdout.strip()
        if not TAG_PATTERN.fullmatch(tag):
            fail(f"Latest published release tag must match vX.Y.Z: {tag}")
        version = tag.removeprefix("v")
    write_github_output(args.github_output, version=version)


def deploy_web(args: argparse.Namespace) -> None:
    version = validate_version(args.version)
    token = require_environment("VERCEL_TOKEN")
    require_environment("VERCEL_ORG_ID")
    require_environment("VERCEL_PROJECT_ID")
    env = os.environ.copy()
    env["PUBLIC_DESKTOP_FUSHI_VERSION"] = version
    run(["npm", "ci"], cwd=WEB_DIR, env=env)
    run(
        ["npx", "vercel@48.12.0", "pull", "--yes", "--environment=production", f"--token={token}"],
        cwd=WEB_DIR,
        env=env,
        redact=(token,),
    )
    run(
        ["npx", "vercel@48.12.0", "build", "--prod", f"--token={token}"],
        cwd=WEB_DIR,
        env=env,
        redact=(token,),
    )
    run(
        ["npx", "vercel@48.12.0", "deploy", "--prebuilt", "--prod", f"--token={token}"],
        cwd=WEB_DIR,
        env=env,
        redact=(token,),
    )


def parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Desktop Fushi CD commands")
    commands = parser.add_subparsers(dest="command", required=True)

    metadata_parser = commands.add_parser("metadata", help="Validate a release tag and read its version")
    metadata_parser.add_argument("--tag", required=True)
    metadata_parser.add_argument("--github-output", type=Path)
    metadata_parser.set_defaults(func=metadata)

    windows_parser = commands.add_parser("package-windows", help="Package a Windows executable")
    windows_parser.add_argument("--version", required=True)
    windows_parser.add_argument("--arch", choices=sorted(WINDOWS_TARGETS), required=True)
    windows_parser.add_argument("--target")
    windows_parser.add_argument("--dist-dir", type=Path, default=ROOT / "dist")
    windows_parser.set_defaults(func=package_windows)

    macos_parser = commands.add_parser("package-macos", help="Package macOS application bundles")
    macos_parser.add_argument("--version", required=True)
    macos_parser.add_argument("--dist-dir", type=Path, default=ROOT / "dist")
    macos_parser.set_defaults(func=package_macos)

    keystore_parser = commands.add_parser("prepare-android-keystore", help="Decode the Android signing keystore")
    keystore_parser.add_argument("--output", type=Path, required=True)
    keystore_parser.add_argument("--github-env", type=Path)
    keystore_parser.set_defaults(func=prepare_android_keystore)

    android_parser = commands.add_parser("sign-android", help="Sign and verify the Android release APK")
    android_parser.add_argument("--version", required=True)
    android_parser.add_argument("--sdk-root", type=Path, required=True)
    android_parser.add_argument("--build-tools-version", default="35.0.0")
    android_parser.add_argument("--keystore", type=Path, required=True)
    android_parser.add_argument("--dist-dir", type=Path, default=ROOT / "dist")
    android_parser.set_defaults(func=sign_android)

    verify_parser = commands.add_parser("verify-assets", help="Verify the release asset set and archive contents")
    verify_parser.add_argument("--version", required=True)
    verify_parser.add_argument("--dist-dir", type=Path, default=ROOT / "dist")
    verify_parser.set_defaults(func=verify_assets)

    notes_parser = commands.add_parser("write-release-notes", help="Write deterministic GitHub release notes")
    notes_parser.add_argument("--version", required=True)
    notes_parser.add_argument("--output", type=Path, required=True)
    notes_parser.set_defaults(func=write_release_notes)

    publish_parser = commands.add_parser("publish-github", help="Create or update a GitHub Release")
    publish_parser.add_argument("--tag", required=True)
    publish_parser.add_argument("--version", required=True)
    publish_parser.add_argument("--dist-dir", type=Path, default=ROOT / "dist")
    publish_parser.add_argument("--notes", type=Path, required=True)
    publish_parser.add_argument("--repo")
    publish_parser.set_defaults(func=publish_github)

    version_parser = commands.add_parser("resolve-web-version", help="Resolve the public website release version")
    version_parser.add_argument("--requested-version")
    version_parser.add_argument("--github-output", type=Path)
    version_parser.add_argument("--repo")
    version_parser.set_defaults(func=resolve_web_version)

    deploy_parser = commands.add_parser("deploy-web", help="Build and deploy the production website")
    deploy_parser.add_argument("--version", required=True)
    deploy_parser.set_defaults(func=deploy_web)
    return parser


def main() -> None:
    args = parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
