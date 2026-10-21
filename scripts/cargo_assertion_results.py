"""Complete named Cargo inventory with native assertion-site attribution.

The native exit and summary authority stays in cargo_test_results. This layer
accepts failures only at an assertion macro's exact source line and column.
"""
from pathlib import Path
import re

from cargo_test_results import parse


def names(output):
    output = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', output)
    return tuple(sorted(re.findall(r'^test (\S+) \.\.\. (?:ok|FAILED)$', output, re.M)))


def validate(status, output, *, root, sources, expected=None):
    if not isinstance(output, str):
        return parse(status, output)
    output = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', output)
    result = parse(status, output)
    inventory = names(output)
    summaries = re.findall(r'^test result: (?:ok|FAILED)\. \d+ passed; \d+ failed; (\d+) ignored; (\d+) measured;', output, re.M)
    valid = (result.valid and summaries == [('0', '0')]
             and len(inventory) == result.passed + result.failed
             and len(set(inventory)) == len(inventory)
             and (expected is None or inventory == tuple(sorted(expected))))
    blocks = re.findall(r'^---- (\S+) stdout ----\n(.*?)(?=^---- |^failures:|^test result:|\Z)', output, re.M | re.S)
    allowed = {Path(source).resolve() for source in sources}
    if result.failed:
        valid = valid and len(blocks) == result.failed and {name for name, _ in blocks} == set(result.failure_names)
        for _, block in blocks:
            sites = re.findall(r'panicked at ([^\n]+?):(\d+):(\d+):', block)
            if len(sites) != 1:
                valid = False
                continue
            path, line, column = sites[0]
            source = Path(path)
            source = (source if source.is_absolute() else Path(root) / source).resolve()
            if source not in allowed:
                valid = False
                continue
            try:
                lines = source.read_text().splitlines()
            except OSError:
                valid = False
                continue
            if not 1 <= int(line) <= len(lines):
                valid = False
                continue
            text = lines[int(line) - 1]
            if (not re.match(r'\s*assert(?:_eq|_ne)?!', text)
                    or int(column) != len(text) - len(text.lstrip()) + 1):
                valid = False
    return result._replace(valid=bool(valid))
