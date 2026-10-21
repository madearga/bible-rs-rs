#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 madearga
"""Collects the real licence texts of the software embedded in the bundle.

The collector resolves the runtime dependency closure of the app binary for a
target, copies the licence files each crate actually ships, and copies the
licence files of the native libraries and embedded fallback fonts. It writes an
INDEX.md next to the texts that names the crate, version, declared licence, and
the file that carries the text.

A crate that declares a standard licence but ships no licence file gets the
standard text of that licence with a header saying the crate shipped none. No
copyright holder is invented, and the gap is recorded in the index rather than
dropped. A crate whose declared licence has no text at all stops collection
with a non-zero exit, so the bundle step fails instead of shipping a gap.

The output is deterministic for the same lock file and registry.

    python3 scripts/collect-licenses.py --output LICENSES
    python3 scripts/collect-licenses.py --check --output LICENSES
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tarfile

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

LICENSE_FILE_PREFIXES = ("license", "licence", "copying", "notice", "unlicense")

# The licence identifiers the collector can write from a standard text when a
# crate declares the licence but ships no file of its own.
STANDARD_LICENSES = {
    "MIT",
    "Apache-2.0",
    "Zlib",
    "ISC",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "0BSD",
    "Unlicense",
    "BSL-1.0",
    "Unicode-3.0",
    "OFL-1.1",
    "Ubuntu-font-1.0",
}

# The marker each standard text contains, longest first.
LICENSE_MARKERS = [
    ("OFL-1.1", "SIL OPEN FONT LICENSE"),
    ("Ubuntu-font-1.0", "UBUNTU FONT LICENCE"),
    ("Unicode-3.0", "UNICODE LICENSE V3"),
    ("BSL-1.0", "Boost Software License"),
    ("Unlicense", "free and unencumbered software released into the public domain"),
    ("Apache-2.0", "Apache License"),
    ("MIT", "Permission is hereby granted, free of charge"),
    ("0BSD", "for any purpose with or without fee is hereby granted"),
    ("ISC", "Permission to use, copy, modify, and/or distribute this software"),
    ("Zlib", "This software is provided 'as-is'"),
    ("Zlib", 'This software is provided "as-is"'),
    ("BSD-3-Clause", "Neither the name of"),
    ("BSD-2-Clause", "Redistributions of source code must retain"),
]

# The licence files of this project and of the two reading faces. They are
# copied into the output so the folder stands on its own.
PROJECT_FILES = [
    ("LICENSE", "the MIT licence of this program"),
    ("assets/fonts/OFL-Inter.txt", "the Open Font License for Inter"),
    ("assets/fonts/OFL-Literata.txt", "the Open Font License for Literata"),
]

# The native libraries the Rust crates link. The crate's own Cargo manifest
# does not name the upstream C licence, so its actual files are copied here.
NATIVE_FILES = [
    ("zstd-sys", "LICENSE.BSD-3-Clause", "the BSD licence of the upstream zstd C library"),
    ("zstd-sys", "LICENSE", "the upstream zstd C library licence notice"),
]

# The fallback faces epaint_default_fonts embeds behind the reading faces.
FALLBACK_FONT_FILES = [
    ("Hack-Regular.txt", "Hack, MIT with the Bitstream Vera licence"),
    ("OFL.txt", "Noto Emoji, SIL Open Font License 1.1"),
    ("UFL.txt", "Ubuntu, Ubuntu Font Licence 1.0"),
    ("emoji-icon-font-mit-license.txt", "emoji-icon-font, MIT"),
]


def run(command: list[str]) -> str:
    result = subprocess.run(command, capture_output=True, text=True, check=True)
    return result.stdout


def host_target() -> str:
    for line in run(["rustc", "-vV"]).splitlines():
        if line.startswith("host: "):
            return line.split(": ", 1)[1].strip()
    raise SystemExit("cannot read the host target from rustc -vV")


def is_license_file(path: pathlib.Path) -> bool:
    return path.is_file() and path.name.lower().startswith(LICENSE_FILE_PREFIXES)


def classify(text: str) -> str | None:
    for identifier, marker in LICENSE_MARKERS:
        if marker in text:
            if identifier == "Apache-2.0" and "Version 2.0, January 2004" not in text:
                continue
            if identifier == "MIT" and "The above copyright notice" not in text:
                continue
            if identifier == "Zlib" and "altered source versions" not in text.lower():
                continue
            if identifier in {"ISC", "0BSD", "BSD-2-Clause"} and "THE SOFTWARE IS PROVIDED" not in text:
                continue
            return identifier
    return None


def closure(metadata: dict) -> list[dict]:
    """The crates linked into the binary, not build-only dependencies."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    app = next(
        package["id"]
        for package in metadata["packages"]
        if package["name"] == "bible-app" and package.get("source") is None
    )
    seen: set[str] = set()
    stack = [app]
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dependency in nodes[current]["deps"]:
            kinds = {kind["kind"] for kind in dependency["dep_kinds"]}
            if kinds & {None, "normal"}:
                stack.append(dependency["pkg"])
    return sorted(
        (packages[identifier] for identifier in seen if packages[identifier].get("source")),
        key=lambda package: (package["name"], package["version"]),
    )


def runtime_packages(target: str | None = None) -> list[dict]:
    """The crates in the binary's runtime closure for a target."""
    target = target or host_target()
    metadata = json.loads(
        run(
            [
                "cargo",
                "metadata",
                "--format-version",
                "1",
                "--locked",
                "--offline",
                "--filter-platform",
                target,
            ]
        )
    )
    return closure(metadata)


def cargo_home() -> pathlib.Path:
    return pathlib.Path(os.environ.get("CARGO_HOME", pathlib.Path.home() / ".cargo"))


def registry_sources() -> list[pathlib.Path]:
    sources = cargo_home() / "registry" / "src"
    return sorted(child for child in sources.glob("*/*") if child.is_dir()) if sources.is_dir() else []


def crate_directory(package: dict) -> pathlib.Path | None:
    for directory in registry_sources():
        if directory.name == f"{package['name']}-{package['version']}":
            return directory
    return None


def extracted_license_files(package: dict) -> dict[str, bytes]:
    """The licence files the extracted crate ships, by file name."""
    found = {}
    directory = crate_directory(package)
    if directory is not None:
        for child in sorted(directory.iterdir()):
            if is_license_file(child):
                found[child.name] = child.read_bytes()
        # epaint_default_fonts keeps the licence texts beside the faces it
        # embeds rather than at the crate root.
        fonts = directory / "fonts"
        if fonts.is_dir():
            for child in sorted(fonts.iterdir()):
                if child.is_file() and child.suffix.lower() == ".txt":
                    found[child.name] = child.read_bytes()
    return found


def archive_license_files(package: dict) -> dict[str, bytes]:
    """The licence files inside the crate archive, when the crate is not extracted."""
    cache = cargo_home() / "registry" / "cache"
    if not cache.is_dir():
        return {}
    matches = sorted(cache.glob(f"*/{package['name']}-{package['version']}.crate"))
    if not matches:
        return {}
    found = {}
    with tarfile.open(matches[0]) as handle:
        for member in handle.getmembers():
            if not member.isfile():
                continue
            parts = pathlib.PurePosixPath(member.name).parts
            if len(parts) == 2 and parts[1].lower().startswith(LICENSE_FILE_PREFIXES):
                extracted = handle.extractfile(member)
                if extracted is not None:
                    found[parts[1]] = extracted.read()
    return found


def standard_texts() -> dict[str, bytes]:
    """The standard licence texts available in the local registry.

    A text is accepted only when its own words identify the licence. The copy
    that the most distinct crates ship wins, so the text is the common one.
    """
    counts: dict[bytes, set[str]] = {}
    for directory in registry_sources():
        crate = directory.name.rsplit("-", 1)[0]
        for child in directory.iterdir():
            if not is_license_file(child):
                continue
            try:
                data = child.read_bytes()
            except OSError:
                continue
            counts.setdefault(data, set()).add(crate)
    best: dict[str, bytes] = {}
    for data, crates in sorted(counts.items(), key=lambda item: len(item[1]), reverse=True):
        identifier = classify(data.decode("utf-8", errors="replace"))
        if identifier and identifier in STANDARD_LICENSES and identifier not in best:
            best[identifier] = data
    return best


def declared_licences(package: dict) -> list[str]:
    raw = package.get("license") or ""
    identifiers = set()
    for chunk in re.split(r"[\s/(),]+", raw):
        base = chunk.strip().split(" WITH ", 1)[0]
        if base in STANDARD_LICENSES:
            identifiers.add(base)
    return sorted(identifiers)


def notice_name(package: dict, filename: str) -> str:
    return f"{package['name']}-{package['version']}-{filename}"


def collect(output: pathlib.Path, packages: list[dict], standard: dict[str, bytes]) -> tuple[list[tuple], list[str]]:
    rows: list[tuple[str, str, str, str]] = []
    missing: list[str] = []
    for package in packages:
        name, version = package["name"], package["version"]
        declared = package.get("license") or "none"
        files = extracted_license_files(package)
        if not files:
            files = archive_license_files(package)
        written = []
        for filename, data in files.items():
            target = output / notice_name(package, filename)
            target.write_bytes(data)
            written.append(target.name)
        if not written:
            for identifier in declared_licences(package):
                text = standard.get(identifier)
                if text is None:
                    missing.append(
                        f"{name} {version}: declares {identifier} but ships no text and no standard {identifier} text was found"
                    )
                    continue
                header = (
                    f"{name} {version}\n"
                    f"Declared licence: {identifier}\n"
                    f"Upstream: {package.get('repository') or 'not recorded in Cargo.toml'}\n\n"
                    f"The crate archive ships no licence file for this licence. The text below is the\n"
                    f"standard {identifier} text. No copyright holder is stated here because the crate\n"
                    f"does not ship one. Check the crate's own repository before relying on this file.\n\n"
                    f"{'-' * 72}\n\n"
                ).encode()
                target = output / notice_name(package, f"{identifier}.txt")
                target.write_bytes(header + text)
                written.append(target.name)
            if not declared_licences(package):
                missing.append(f"{name} {version}: declares no licence identifier")
        if written:
            rows.append((name, version, declared, ", ".join(f"`{item}`" for item in sorted(written))))
        elif not any(item.startswith(f"{name} {version}:") for item in missing):
            missing.append(f"{name} {version}: no licence text could be collected")
    return rows, missing


def extra_files(output: pathlib.Path) -> list[tuple[str, str]]:
    """Copies the project, native, and fallback-font notices."""
    written = []
    for relative, description in PROJECT_FILES:
        source = REPO_ROOT / relative
        if not source.is_file():
            raise SystemExit(f"missing project licence file: {source}")
        target = output / pathlib.Path(relative).name
        shutil.copy2(source, target)
        written.append((target.name, description))
    for crate, filename, description in NATIVE_FILES:
        matches = [directory for directory in registry_sources() if directory.name.startswith(f"{crate}-")]
        if not matches:
            raise SystemExit(f"the {crate} source is not in the local registry, so {filename} cannot be copied")
        source = matches[-1] / filename
        if not source.is_file():
            raise SystemExit(f"the {crate} crate does not ship {filename}")
        target = output / f"{crate}-{filename}"
        shutil.copy2(source, target)
        written.append((target.name, description))
    matches = [directory for directory in registry_sources() if directory.name.startswith("epaint_default_fonts-")]
    if not matches:
        raise SystemExit("the epaint_default_fonts source is not in the local registry")
    for filename, description in FALLBACK_FONT_FILES:
        source = matches[-1] / "fonts" / filename
        if not source.is_file():
            raise SystemExit(f"epaint_default_fonts does not ship fonts/{filename}")
        target = output / f"fallback-fonts-{filename}"
        shutil.copy2(source, target)
        written.append((target.name, description))
    return written


def write_index(output: pathlib.Path, rows: list[tuple], extras: list[tuple]) -> None:
    lines = [
        "# Licence texts shipped with Bible.app",
        "",
        "This folder is written by `scripts/collect-licenses.py` from the crate",
        "archives in the local Cargo registry, the native sources those crates",
        "link, and the fallback fonts the GUI toolkit embeds. `INDEX.md` names what",
        "each file covers. This is a record of the texts that travel with the",
        "binary, not a legal opinion.",
        "",
        "## Rust crates in the runtime closure",
        "",
        "| Crate | Version | Declared licence | Notice files |",
        "|---|---|---|---|",
    ]
    for row in rows:
        lines.append("| `{}` | {} | {} | {} |".format(*row))
    lines += [
        "",
        "## Project, native library, and fallback-font notices",
        "",
        "| File | What it covers |",
        "|---|---|",
    ]
    for filename, description in extras:
        lines.append(f"| `{filename}` | {description} |")
    lines.append("")
    (output / "INDEX.md").write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=str(REPO_ROOT / "LICENSES"))
    parser.add_argument("--target", default=None, help="the target triple, defaulting to the host")
    parser.add_argument("--check", action="store_true", help="verify the folder without writing to it")
    args = parser.parse_args()

    output = pathlib.Path(args.output)

    # The check resolves the current runtime closure and compares every
    # dependency identity against the index, so a crate that enters the binary
    # without a notice fails the check. It also verifies that every file the
    # index names is present.
    if args.check:
        index = output / "INDEX.md"
        if not index.is_file():
            print(f"missing notice index: {index}", file=sys.stderr)
            return 1
        missing = []
        indexed = set()
        rows = 0
        for line in index.read_text(encoding="utf-8").splitlines():
            if not line.startswith("| `"):
                continue
            cells = line.split("|")
            if len(cells) < 5:
                continue
            rows += 1
            crate = re.findall(r"`([^`]+)`", cells[1])
            version = cells[2].strip()
            if len(crate) == 1:
                indexed.add((crate[0], version))
            for name in re.findall(r"`([^`]+)`", cells[4]):
                if not (output / name).is_file():
                    missing.append(name)
        if rows == 0:
            missing.append("INDEX.md names no crate notices")
        try:
            packages = runtime_packages(args.target)
        except (subprocess.CalledProcessError, SystemExit) as error:
            print(f"cannot resolve the runtime dependency closure: {error}", file=sys.stderr)
            return 1
        for package in packages:
            identity = (package["name"], package["version"])
            if identity not in indexed:
                missing.append(
                    f"{package['name']} {package['version']}: in the runtime closure but absent from INDEX.md"
                )
        for filename, _description in [(pathlib.Path(path).name, d) for path, d in PROJECT_FILES]:
            if not (output / filename).is_file():
                missing.append(filename)
        for required in (
            "zstd-sys-LICENSE.BSD-3-Clause",
            "fallback-fonts-Hack-Regular.txt",
            "fallback-fonts-OFL.txt",
            "fallback-fonts-UFL.txt",
            "fallback-fonts-emoji-icon-font-mit-license.txt",
        ):
            if not (output / required).is_file():
                missing.append(required)
        if missing:
            for name in sorted(set(missing)):
                print(f"missing notice: {name}", file=sys.stderr)
            print(f"{len(set(missing))} required notices are missing from {output}", file=sys.stderr)
            return 1
        print(f"every notice named by {index} is in {output}")
        return 0

    packages = runtime_packages(args.target)
    standard = standard_texts()

    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    rows, missing = collect(output, packages, standard)
    extras = extra_files(output)
    write_index(output, rows, extras)
    print(f"collected notices for {len(rows)} crates into {output}")
    if missing:
        for item in missing:
            print(f"unresolved licence: {item}", file=sys.stderr)
        print(f"{len(missing)} required licence texts could not be collected", file=sys.stderr)
        return 1
    print("every crate in the runtime closure has a licence text")
    return 0


if __name__ == "__main__":
    sys.exit(main())
