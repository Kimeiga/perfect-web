#!/usr/bin/env python3
"""Exercise the pinned Playwright reporter with native no-browser test children."""
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from browser_test_results import parse


def main():
    spike = ROOT / 'spikes/own-renderer'
    package = subprocess.check_output(['node', '-p', "require.resolve('@playwright/test/package.json')"], cwd=spike, text=True).strip()
    package = Path(package)
    if json.loads(package.read_text())['version'] != '1.58.0':
        raise RuntimeError('reporter protocol must be rechecked for a new Playwright pin')
    with tempfile.TemporaryDirectory(prefix='pw-browser-protocol-') as tmp:
        tmp = Path(tmp)
        source = f'''import {{ test, expect }} from {json.dumps((package.parent / 'index.mjs').as_uri())};
const mode = process.env.PROTOCOL_MODE;
if (mode === 'before') test.beforeEach(() => expect(1).toBe(2));
if (mode === 'setupTimeout') test.beforeEach(async () => {{ test.setTimeout(10); await new Promise(r => setTimeout(r, 100)); }});
if (mode === 'after') test.afterEach(() => expect(1).toBe(2));
test('witness', async () => {{
  if (mode === 'assertion') expect(1).toBe(2);
  if (mode === 'runtime') throw new Error('runtime failure');
  if (mode === 'timeout') {{ test.setTimeout(10); await new Promise(r => setTimeout(r, 100)); }}
  if (mode === 'skipped') test.skip();
  if (mode === 'retry') expect(test.info().retry).toBe(1);
  expect(1).toBe(1);
}});
'''
        (tmp / 'fixture.spec.mjs').write_text(source)
        (tmp / 'config.mjs').write_text('export default ' + json.dumps({'testDir': str(tmp), 'workers': 1,
            'reporter': [['line'], [str(spike / 'assertion-result-reporter.mjs')]]}))
        for mode in ('green', 'assertion', 'before', 'after', 'runtime', 'timeout', 'setupTimeout', 'skipped', 'retry'):
            nonce = secrets.token_hex(32)
            report = tmp / (mode + '.json')
            run = subprocess.run(['node', str(package.parent / 'cli.js'), 'test', '--config', str(tmp / 'config.mjs'),
                                  '--retries=' + ('1' if mode == 'retry' else '0')], cwd=spike,
                env={**os.environ, 'PROTOCOL_MODE': mode, 'PW_ASSERTION_REPORT': str(report), 'PW_ASSERTION_NONCE': nonce},
                capture_output=True, text=True, timeout=30)
            receipt = json.loads(report.read_text()) if report.exists() else None
            result = parse(run.returncode, receipt, nonce)
            expected = mode in ('green', 'assertion')
            print(f'{mode}: native exit {run.returncode}, valid={result.valid}, expected={expected}', flush=True)
            if result.valid != expected or (expected and result.failed != int(mode == 'assertion')):
                print(run.stdout + run.stderr)
                return 1
            if mode == 'assertion':
                # Preserve the ordinary native red assertion, as the campaign does.
                print(run.stdout + run.stderr)
    print('9 of 9 native Playwright producer cases agree')
    return 0


if __name__ == '__main__':
    sys.exit(main())
