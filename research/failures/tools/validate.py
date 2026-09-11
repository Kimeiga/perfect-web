#!/usr/bin/env python3
"""Check census structure and derive JSON. This does not certify semantic claims."""
from __future__ import annotations
import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STATUSES = {"covered", "partially covered", "specified but unproven", "missing", "intentionally outside scope"}
FIELDS = ("id", "title", "example", "root", "existing", "status", "layer", "treatment", "dx", "evidence")
LEAF = re.compile(r"[A-Z]+[0-9]{2,}\Z")
REF = re.compile(r"\[([SR][0-9]+)\]")


def load_census(root: Path = ROOT) -> str:
    """Assemble only the chapters explicitly linked by the canonical index."""
    index = (root / "CENSUS.md").read_text()
    chapters = re.findall(r"\]\((taxonomy/[a-z0-9-]+\.md)\)", index)
    if not chapters or len(chapters) != len(set(chapters)):
        raise ValueError("CENSUS.md: missing or duplicate taxonomy chapter")
    listed = {(root / name).resolve() for name in chapters}
    present = {path.resolve() for path in (root / "taxonomy").glob("*.md")}
    if listed != present:
        raise ValueError("CENSUS.md: taxonomy chapters do not match the on-disk inventory")
    return index + "\n" + "\n".join((root / name).read_text() for name in chapters)


def parse_census(text: str) -> dict:
    """The Markdown table is the only editable classification authority."""
    revision = re.search(r"Audited repository revision:\*\* `([0-9a-f]{40})`", text)
    if not revision:
        raise ValueError("CENSUS.md: missing audited commit SHA")
    groups: dict[str, str] = {}
    rows: list[dict] = []
    ids: set[str] = set()
    roots: set[str] = set()
    links = dict(re.findall(r"^\[([SR][0-9]+)\]: (\S+)$", text, re.M))
    group = None
    in_table = False
    for line_number, line in enumerate(text.splitlines(), 1):
        heading = re.match(r"## ([A-Z]+)\. (.+)$", line)
        if heading:
            group, title = heading.groups()
            if group in groups:
                raise ValueError(f"line {line_number}: duplicate group {group}")
            groups[group] = title
        if line.startswith("| ID | Failure class |"):
            in_table = True
            continue
        if not line.startswith("|"):
            in_table = False
            continue
        if not in_table or re.fullmatch(r"[|:\- ]+", line):
            continue
        cells = [part.strip() for part in line.strip().strip("|").split("|")]
        if len(cells) != len(FIELDS):
            raise ValueError(f"line {line_number}: expected {len(FIELDS)} cells, got {len(cells)}")
        row = dict(zip(FIELDS, cells))
        if any(not value for value in row.values()):
            raise ValueError(f"line {line_number}: empty census field")
        if not LEAF.fullmatch(row["id"]) or re.match(r"[A-Z]+", row["id"]).group() != group:
            raise ValueError(f"line {line_number}: leaf outside its group")
        if row["id"] in ids:
            raise ValueError(f"line {line_number}: duplicate ID {row['id']}")
        ids.add(row["id"])
        normalized = " ".join(row["root"].lower().split())
        if normalized in roots:
            raise ValueError(f"line {line_number}: duplicate root invariant")
        roots.add(normalized)
        if row["status"] not in STATUSES:
            raise ValueError(f"line {line_number}: unknown coverage status")
        if not re.fullmatch(r"[A-I](?:\+[A-I])*", row["layer"]):
            raise ValueError(f"line {line_number}: invalid enforcement layer")
        if not re.fullmatch(r"[TERUXDPV](?:\+[TERUXDPV])*", row["dx"]):
            raise ValueError(f"line {line_number}: invalid DX profile")
        refs = REF.findall(row["evidence"])
        if not refs or any(ref not in links for ref in refs):
            raise ValueError(f"line {line_number}: unresolved evidence reference")
        row["evidence"] = " ".join(refs)
        row["group"] = group
        rows.append(row)
    if not rows:
        raise ValueError("CENSUS.md: empty failure inventory")
    for key in groups:
        if not any(row["group"] == key for row in rows):
            raise ValueError(f"empty group {key}")
    return {"generated_from": "CENSUS.md and its linked taxonomy chapters", "audited_revision": revision.group(1), "groups": groups, "failures": rows}


def validate_obligations(text: str, leaf_ids: set[str]) -> dict[str, int]:
    counts = Counter()
    seen: set[str] = set()
    for line_number, line in enumerate(text.splitlines(), 1):
        match = re.match(r"\| ((CF|RT)[0-9]+)\s*\|", line)
        if not match:
            continue
        identifier, kind = match.groups()
        if identifier in seen:
            raise ValueError(f"obligations line {line_number}: duplicate {identifier}")
        seen.add(identifier)
        cells = [part.strip() for part in line.strip().strip("|").split("|")]
        expected = 5 if kind == "CF" else 3
        if len(cells) != expected:
            raise ValueError(f"obligations line {line_number}: expected {expected} cells, got {len(cells)}")
        refs = cells[-1].split()
        if not refs or any(ref not in leaf_ids for ref in refs):
            raise ValueError(f"obligations line {line_number}: unresolved census leaf")
        if kind == "CF" and cells[1] not in {"S", "P"}:
            raise ValueError(f"obligations line {line_number}: source/plan lane required")
        counts[kind] += 1
    if not counts["CF"] or not counts["RT"]:
        raise ValueError("both compile-fail and runtime obligations are required")
    return dict(counts)


def validate(root: Path = ROOT) -> dict:
    data = parse_census(load_census(root))
    counts = validate_obligations((root / "OBLIGATIONS.md").read_text(), {r["id"] for r in data["failures"]})
    source_text = (root / "SOURCES.md").read_text()
    registry = set(re.findall(r"^#{2,3} ([SR][0-9]+)(?::|$)", source_text, re.M))
    for row in data["failures"]:
        if not set(row["evidence"].split()) <= registry:
            raise ValueError(f"{row['id']}: evidence missing from SOURCES.md")
    for name in ("README.md", "CENSUS.md", "OBLIGATIONS.md", "REQUIREMENTS.md", "SOURCES.md"):
        text = (root / name).read_text()
        if any(ref not in registry for ref in REF.findall(text)):
            raise ValueError(f"{name}: reference absent from the evidence registry")
    # If an export is kept locally, it must be exactly derived. It is optional.
    export = root / "census.json"
    if export.exists() and json.loads(export.read_text()) != data:
        raise ValueError("census.json drifted; regenerate with --export, do not edit it")
    return {"data": data, "obligations": counts, "sources": len(registry)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--export", type=Path, help="write a derived JSON export (does not change Markdown)")
    args = parser.parse_args()
    try:
        data = parse_census(load_census())
        if args.export:
            if args.export.resolve() in {p.resolve() for p in ROOT.rglob("*.md")}:
                raise ValueError("export cannot overwrite an authoritative Markdown source")
            args.export.write_text(json.dumps(data, indent=2) + "\n")
        result = validate()
    except (OSError, ValueError) as error:
        print(f"census validation failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"failures": len(data["failures"]), "families": len(data["groups"]),
        "coverage": dict(Counter(row["status"] for row in data["failures"])),
        "obligations": result["obligations"], "evidence_anchors": result["sources"],
        "claim": "structural consistency only; not proof of implementation"}, indent=2))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
