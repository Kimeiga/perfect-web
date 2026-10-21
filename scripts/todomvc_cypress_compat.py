#!/usr/bin/env python3
"""Generate/verify the exact v1 execution copy; never edit upstream sources."""

import argparse
import hashlib
import json
import pathlib
import sys

import todomvc_cypress_vendored as upstream

IDENTITY = "todomvc-cypress-compat-v1"
REVISION = "ff43b02e59dfa604386bb382034b2cd07c2bcd8a"
MANIFEST = "compatibility-manifest.json"
PATCHES = [
    ("spy-reset-history", "cypress/e2e/spec.cy.js",
     ".invoke('reset')", ".invoke('resetHistory')"),
    ("fresh-created-todo-query", "cypress/support/commands.js",
     "        cmd.set({ $el: $li }).snapshot().end()\n      })\n  })",
     "        cmd.set({ $el: $li }).snapshot().end()\n      })\n"
     "    return cy.get(todoListSelector, { log: false }).contains('li', todo.trim(), { log: false })\n  })"),
]
# A changed patch or result needs a reviewed new identity, not a silently
# different adapter with the same name. The source authority is upstream.BLOBS.
PATCHSET_SHA256 = "25d26eb59c020b9ea261f65861f0b7dad4cb698fce1ade1d3f4b9276f03ed3ee"
PATCHED_SHA256 = {
    "cypress/e2e/spec.cy.js": "c353401d93731945f77c3f8341fcded3bb5479b2d36944349133a72ac09cd88c",
    "cypress/support/commands.js": "4aa73053c8323b9bf93fdbf706147968d2f43627e6d4366e8c86cddbd580d843",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def regular(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"missing or nonregular file: {path}")
    return path.read_bytes()


def inputs(source, package_version, binary_version):
    for label, version in (("package", package_version), ("binary", binary_version)):
        if version != upstream.CYPRESS:
            raise ValueError(f"Cypress {label} {version!r} is not pinned {upstream.CYPRESS}")
    files = {}
    for name, expected in upstream.BLOBS.items():
        data = regular(source / name)
        if upstream.blob(data) != expected:
            raise ValueError(f"upstream blob drift: {name}")
        files[name] = data
    package = regular(source / "package.json")
    if json.loads(package).get("devDependencies", {}).get("cypress") != upstream.CYPRESS:
        raise ValueError("declared Cypress package pin drift")
    files["package.json"] = package
    return files


def expected(source, package_version, binary_version):
    files = inputs(source, package_version, binary_version)
    patches = json.dumps(PATCHES, separators=(",", ":")).encode()
    if digest(patches) != PATCHSET_SHA256:
        raise ValueError("compatibility patch identity drift")
    for _, name, before, after in PATCHES:
        before, after = before.encode(), after.encode()
        if files[name].count(before) != 1:
            raise ValueError(f"compatibility anchor is not unique: {name}")
        files[name] = files[name].replace(before, after)
    for name, wanted in PATCHED_SHA256.items():
        if digest(files[name]) != wanted:
            raise ValueError(f"compatibility output identity drift: {name}")
    manifest = {
        "identity": IDENTITY,
        "upstream_revision": REVISION,
        "cypress_package": package_version,
        "cypress_binary": binary_version,
        "upstream_blobs": upstream.BLOBS,
        "patchset_sha256": PATCHSET_SHA256,
        "patches": [name for name, _, _, _ in PATCHES],
        "output_sha256": {name: digest(data) for name, data in sorted(files.items())},
    }
    files[MANIFEST] = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    return files


def verify(destination, package_version, binary_version, *, source=upstream.DIR):
    files = expected(source, package_version, binary_version)
    if destination.is_symlink() or not destination.is_dir():
        raise ValueError("staged project is missing or a symlink")
    entries = list(destination.rglob("*"))
    if any(path.is_symlink() for path in entries):
        raise ValueError("staged project contains a symlink")
    actual = {path.relative_to(destination).as_posix() for path in entries if not path.is_dir()}
    if actual != set(files):
        raise ValueError("staged file inventory drift")
    for name, data in files.items():
        if regular(destination / name) != data:
            raise ValueError(f"staged content drift: {name}")
    return digest(files[MANIFEST])


def create(destination, package_version, binary_version, *, source=upstream.DIR):
    files = expected(source, package_version, binary_version)
    # Refuse existing paths; this tool never overwrites or removes user files.
    destination.mkdir()
    for name, data in files.items():
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    return verify(destination, package_version, binary_version, source=source)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("create", "verify"))
    parser.add_argument("destination", type=pathlib.Path)
    parser.add_argument("--package-version", required=True)
    parser.add_argument("--binary-version", required=True)
    args = parser.parse_args(argv)
    try:
        manifest = globals()[args.mode](args.destination, args.package_version, args.binary_version)
    except (OSError, ValueError) as error:
        print(f"{IDENTITY}: REFUSED: {error}", file=sys.stderr)
        return 1
    print(f"{IDENTITY}: {args.mode}; original 7 upstream blobs unchanged; "
          f"exact 2-patch execution copy; manifest sha256 {manifest}")
    if args.mode == "create":
        print((args.destination / MANIFEST).read_text(), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
