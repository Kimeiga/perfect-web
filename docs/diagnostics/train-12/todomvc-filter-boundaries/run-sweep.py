"""Run original TodoMVC model entries 10/11 with strict receipts retained."""
import json
import pathlib
import subprocess
import sys
import tempfile as real_tempfile
import types

root = pathlib.Path.cwd()
out = pathlib.Path('/tmp/todomvc-route-boundary-20261021')
sys.path.insert(0, str(root / 'scripts'))
import todomvc_mutations as campaign

original = campaign.MUTANTS
assert len(original) == 43, len(original)
want = {item[0] for index, item in enumerate(original) if 12 <= index <= 20}
selected = [(index, item) for index, item in enumerate(original) if item[0] in want]
assert [index for index, _ in selected] == list(range(12, 21))

class PersistentTemporaryDirectory:
    def __init__(self, prefix):
        self.path = pathlib.Path(real_tempfile.mkdtemp(prefix=prefix, dir=out))
    def __enter__(self):
        return str(self.path)
    def __exit__(self, *_):
        return False

campaign.tempfile = types.SimpleNamespace(TemporaryDirectory=PersistentTemporaryDirectory)
campaign.MUTANTS = [item for _, item in selected]
(out / 'inventory.json').write_text(json.dumps({
    'repo': str(root),
    'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    'full_original_mutant_count': len(original),
    'selected_original_zero_based_indexes': [index for index, _ in selected],
    'selected_original_names': [item[0] for _, item in selected],
    'browser_expected_count': campaign.BROWSER_COUNT,
    'server_expected_names': sorted(campaign.SERVER_NAMES),
    'test_id': 'bounded original indexes 12-20',
}, indent=2) + '\n')
raise SystemExit(campaign.main())
