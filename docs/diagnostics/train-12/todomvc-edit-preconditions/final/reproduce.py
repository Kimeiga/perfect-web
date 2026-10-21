import json, os, pathlib, secrets, sys, time
ROOT = pathlib.Path('/home/haki/code/pw-wt/todomvc-toggle-all-visible-precondition')
sys.path.insert(0, str(ROOT / 'scripts'))
import browser_test_results, cargo_assertion_results, mutation_source, producer_process
import todomvc_mutations as campaign
os.environ['PORT'] = '18541'
os.environ['CARGO_TARGET_DIR'] = '/home/haki/code/pw-wt/todomvc-survivors/target'
os.environ['PATH'] = '/home/haki/.nvm/versions/node/v22.21.1/bin:' + os.environ['PATH']
proof = pathlib.Path('/tmp/train12-todomvc-edit-preconditions-final')
proof.mkdir(exist_ok=False)
expected = None
server_expected = None
start = time.monotonic()

def native(command, label, env=None, cwd=ROOT):
    result = producer_process.run(command, timeout=300, cwd=cwd, env=env or os.environ.copy())
    (proof / (label + '.txt')).write_text(result.stdout)
    return result

def host(label):
    global server_expected
    r = native([x for x in campaign.SERVER_TESTS if x != '--quiet'] + ['--test-threads=1'], label)
    parsed = cargo_assertion_results.validate(r.returncode, r.stdout, root=ROOT, sources=[ROOT / 'spikes/own-renderer/server/src/tests/todomvc.rs'], expected=server_expected)
    print(label, json.dumps({'native_status': r.returncode, 'parsed': parsed._asdict()}, default=list), flush=True)
    assert parsed.valid and parsed.passed + parsed.failed == 4
    if server_expected is None:
        server_expected = cargo_assertion_results.names(r.stdout)
    assert parsed.failed == 0

def browser(label, should_fail=False):
    global expected
    builds = [
        ('pages', ['bash', 'spikes/own-renderer/run.sh'], {**os.environ, 'BUILD_ONLY': '1'}),
        ('todo', ['bash', 'spikes/own-renderer/todomvc.sh'], None),
        ('host', ['cargo', 'build', '--quiet', '--locked', '-p', 'pw-dev-server', '-p', 'kiokun-server'], None),
    ]
    for name, command, env in builds:
        r = native(command, label + '-build-' + name, env)
        assert r.returncode == 0, (name, r.stdout[-3000:])
    target = proof / label
    target.mkdir(exist_ok=False)
    report = target / 'assertions.json'
    nonce = secrets.token_hex(32)
    command = ['./node_modules/.bin/playwright', 'test', 'e2e/todomvc.spec.mjs', '--project=chromium', '--retries=0', '--reporter=line,./assertion-result-reporter.mjs']
    r = producer_process.run(command, timeout=300, cwd=ROOT / 'spikes/own-renderer', env={**os.environ, 'PW_ASSERTION_REPORT': str(report), 'PW_ASSERTION_NONCE': nonce})
    (target / 'output.txt').write_text(r.stdout)
    native_report = json.loads(report.read_text()) if report.exists() else None
    parsed = browser_test_results.parse(r.returncode, native_report, nonce, expected)
    print(label, json.dumps({'native_status': r.returncode, 'valid': parsed.valid, 'reason': parsed.reason, 'passed': parsed.passed, 'failed': parsed.failed, 'elapsed': time.monotonic() - start}), flush=True)
    assert parsed.valid, parsed.reason
    assert parsed.passed + parsed.failed == 50
    if expected is None:
        expected = parsed.inventory
        (proof / 'expected-50-ids.json').write_text(json.dumps(expected, indent=2) + '\n')
    assert parsed.failed > 0 if should_fail else parsed.failed == 0

host('baseline-server')
browser('baseline-browser')
files = sorted((ROOT / 'packages/pw-std').glob('*.pw')) + sorted((ROOT / 'packages/pw-platform-web').glob('*.pw')) + sorted((ROOT / 'examples/todomvc').glob('*.pw'))
check = ['cargo', 'run', '--quiet', '--locked', '-p', 'pw-cli', '--', 'check', '--plain', *map(str, files)]
for i, number in [(15, 16), (29, 30)]:
    name, suite, path, anchor, replacement = campaign.MUTANTS[i]
    assert suite == campaign.PROGRAM, (number, name, suite)
    original = path.read_text()
    assert original.count(anchor) == 1, (number, name)
    with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
        r = native(check, 'operator-' + str(number) + '-check')
        assert r.returncode == 0, (number, r.stdout[-3000:])
        browser('operator-' + str(number), True)
    assert path.read_text() == original
host('restored-server')
browser('restored-browser')
print('PASS: exact4/50 inventories; baseline, original operators16/30, and restored source; elapsed', time.monotonic() - start, flush=True)
