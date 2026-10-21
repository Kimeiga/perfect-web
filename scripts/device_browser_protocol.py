#!/usr/bin/env python3
"""Native pinned Playwright producer proof, including real browser/API failures."""
import json
import os
from pathlib import Path
import secrets
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from browser_test_results import parse
import producer_process

VALID = {'green': 0, 'assertion': 1, 'customMessage': 1, 'not': 1, 'locator': 1,
         'pollGreen': 0, 'poll': 1, 'pollNaN': 0, 'pollSignedZero': 1, 'pollEarlyTimer': 1, 'cause': 1}
INVALID = ('closed', 'invalidMatcher', 'pollCallback', 'pollAssertCallback', 'pollPending',
           'pollOptions', 'pollUnsupported', 'before', 'after', 'runtime', 'timeout',
           'setupTimeout', 'skipped', 'retry', 'soft', 'unwrapped')
BROWSER_MODES = {'closed', 'invalidMatcher', 'locator'}


def native(command, cwd, env):
    """Reap our native child group before timeout/cancellation releases files."""
    result = producer_process.run(command, timeout=30, cwd=cwd, env=env)
    return result.returncode, result.stdout


def run_case(mode):
    spike = ROOT / 'spikes/own-renderer'
    resolved = producer_process.run(['node', '-p', "require.resolve('@playwright/test/package.json')"],
                                    timeout=10, cwd=spike)
    if resolved.returncode != 0:
        raise RuntimeError(resolved.stdout)
    package = Path(resolved.stdout.strip())
    if json.loads(package.read_text())['version'] != '1.58.0':
        raise RuntimeError('reporter protocol must be rechecked for a new Playwright pin')
    with tempfile.TemporaryDirectory(prefix='pw-browser-protocol-') as directory:
        tmp = Path(directory)
        source = ('import { test, expect as nativeExpect } from ' + json.dumps((package.parent / 'index.mjs').as_uri()) + ';\n'
                  'import { expect } from ' + json.dumps((spike / 'assertion-expect.mjs').as_uri()) + ';\n') + r'''
const mode = process.env.PROTOCOL_MODE;
const polling = {timeout: 90, intervals: [10]};
if (mode === 'before') test.beforeEach(() => expect(1).toBe(2));
if (mode === 'setupTimeout') test.beforeEach(async () => { test.setTimeout(10); await new Promise(r => setTimeout(r, 100)); });
if (mode === 'after') test.afterEach(() => expect(1).toBe(2));
async function witness() {
  if (mode === 'assertion') expect(1).toBe(2);
  if (mode === 'customMessage') expect(1, 'custom diagnostic').toBe(2);
  if (mode === 'not') expect(1).not.toBe(1);
  if (mode === 'pollGreen') { let n = 0; await expect.poll(() => ++n, polling).toBe(2); }
  if (mode === 'poll') await expect.poll(() => 'fresh', polling).toBe('stale');
  if (mode === 'pollNaN') await expect.poll(() => NaN, polling).toBe(NaN);
  if (mode === 'pollSignedZero') await expect.poll(() => -0, polling).toBe(0);
  if (mode === 'pollEarlyTimer') {
    // Force timer rounding below the final clamped delay while retaining
    // the real monotonic clock.
    // A second callback is deliberately pending and cannot be assertion proof.
    const originalTimer = globalThis.setTimeout;
    let calls = 0;
    globalThis.setTimeout = (callback, delay, ...args) =>
      originalTimer(callback, delay > 0 && delay <= 100 ? 1 : delay, ...args);
    try {
      await expect.poll(() => ++calls === 1 ? 'fresh' : new Promise(() => {}),
        {timeout: 90, intervals: [100]}).toBe('stale');
    } finally {
      globalThis.setTimeout = originalTimer;
      if (calls !== 1) throw new Error('a final clamped sleep restarted the callback');
    }
  }
  if (mode === 'pollCallback') await expect.poll(() => { throw new Error('callback failure'); }, polling).toBe(undefined);
  if (mode === 'pollAssertCallback') await expect.poll(() => { expect(1).toBe(2); }, polling).toBe(1);
  if (mode === 'pollPending') await expect.poll(() => new Promise(() => {}), polling).toBe(1);
  if (mode === 'pollOptions') await expect.poll(() => 1, {timeout: Infinity}).toBe(2);
  if (mode === 'pollUnsupported') await expect.poll(() => 1, polling).not.toBe(1);
  if (mode === 'cause') {
    const original = new Error('existing native cause');
    Object.defineProperty(Error.prototype, 'cause', {configurable: true, get() {return this === original ? undefined : original;}});
    try { expect(1).toBe(2); }
    catch (error) {
      if (error.cause?.cause !== original) throw new Error('native cause was lost');
      throw error;
    } finally { delete Error.prototype.cause; }
  }
  if (mode === 'runtime') throw new Error('runtime failure');
  if (mode === 'timeout') { test.setTimeout(10); await new Promise(r => setTimeout(r, 100)); }
  if (mode === 'skipped') test.skip();
  if (mode === 'retry') expect(test.info().retry).toBe(1);
  if (mode === 'soft') expect.soft(1).toBe(2);
  if (mode === 'unwrapped') nativeExpect(1).toBe(2);
  expect(1).toBe(1);
}
if (['closed','invalidMatcher','locator'].includes(mode)) {
  test('witness', async ({page}) => {
    await page.setContent('<p>actual</p>');
    if (mode === 'closed') await page.close();
    await expect(mode === 'invalidMatcher' ? 42 : page.locator('p')).toHaveText('expected', {timeout: 90});
  });
} else test('witness', witness);
'''
        (tmp / 'fixture.spec.mjs').write_text(source)
        (tmp / 'config.mjs').write_text('export default ' + json.dumps({'testDir': str(tmp), 'workers': 1,
            'use': {'browserName': 'chromium', 'headless': True},
            'reporter': [['line'], [str(spike / 'assertion-result-reporter.mjs')]]}))
        nonce = secrets.token_hex(32)
        report = tmp / 'result.json'
        status, output = native(['node', str(package.parent / 'cli.js'), 'test', '--config', str(tmp / 'config.mjs'),
                                '--retries=' + ('1' if mode == 'retry' else '0')], spike,
            {**os.environ, 'PROTOCOL_MODE': mode, 'PW_ASSERTION_REPORT': str(report), 'PW_ASSERTION_NONCE': nonce})
        receipt = json.loads(report.read_text()) if report.exists() else None
        return status, parse(status, receipt, nonce), output


def main():
    modes = [mode for mode in (*VALID, *INVALID) if '--browser' in sys.argv or mode not in BROWSER_MODES]
    for mode in modes:
        status, result, output = run_case(mode)
        expected = mode in VALID
        expected_status = 0 if mode in ('green', 'pollGreen', 'pollNaN', 'skipped', 'retry') else 1
        print(f'{mode}: native exit {status}, valid={result.valid}, expected={expected}', flush=True)
        if status != expected_status or result.valid != expected or (expected and result.failed != VALID[mode]):
            print(output)
            return 1
        if mode in ('assertion', 'poll', 'closed', 'invalidMatcher'):
            print(output)
    print(f'{len(modes)} of {len(modes)} native Playwright producer cases agree')
    return 0


if __name__ == '__main__':
    sys.exit(main())
