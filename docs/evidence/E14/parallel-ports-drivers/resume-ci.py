from pathlib import Path
import subprocess,os,json,hashlib,time,importlib.util,sys,re,datetime
ROOT=Path('/home/haki/code/pw-wt/parallel-instrument')
OUT=Path('/tmp/parallel-ports-v1')
expected=json.loads((OUT/'manifest.json').read_text())
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def audit():
    raw=subprocess.run(['git','ls-files','-z','--cached','--others','--exclude-standard'],cwd=ROOT,capture_output=True,check=True).stdout
    names=sorted({x.decode() for x in raw.split(b'\0') if x})
    actual={n:digest(ROOT/n) for n in names if (ROOT/n).is_file()}
    differences=[n for n in sorted(set(actual)|set(expected)) if actual.get(n)!=expected.get(n)]
    index=subprocess.run(['git','ls-files','--stage','-z'],cwd=ROOT,capture_output=True,check=True).stdout
    if index!=(OUT/'index-before').read_bytes(): differences.append('<index>')
    return differences
def state(phase,**more):
    value={'pid':os.getpid(),'phase':phase,'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),**more}
    (OUT/'state.json').write_text(json.dumps(value,indent=2)+'\n');print(json.dumps(value),flush=True)
whole=json.loads((OUT/'whole-receipt.json').read_text())
assert whole['status']==0 and not whole['differences']
assert digest(OUT/'whole85.txt')==whole['raw_sha256']
assert [l for l in (OUT/'whole85.txt').read_text().splitlines() if l.startswith('memory bound:')]==['memory bound: no process was stopped']
assert not json.loads((OUT/'observer-final.json').read_text())['errors']
spec=importlib.util.spec_from_file_location('resume_frozen_protocol',OUT/'unittest_protocol.py');protocol=importlib.util.module_from_spec(spec);spec.loader.exec_module(protocol)
audit_rows=json.loads((OUT/'independent-report-audit.json').read_text())
assert len(audit_rows)==86
for i,row in enumerate(audit_rows):
    report=Path(row['report']);folder=report.parent
    assert digest(report)==row['report_sha256'] and digest(folder/'output.txt')==row['output_sha256']
    ids=json.loads((folder/'expected.json').read_text())
    actual=protocol.validate_result(row['status'],report.read_text(),ids)
    assert actual.valid and row['status']==(0 if i==0 else 1)
    assert list(actual.assertion_failed)==row['assertion_failed'] and list(actual.body_errors)==row['body_errors']
    assert bool(actual.failed)==(i!=0)
assert not audit(),'same-source CI preflight differs'
native=[json.loads(line) for line in (OUT/'native-waits.jsonl').read_text().splitlines()]
inventories=[r for r in native if r['inventory']]
assert len(inventories)==2 and len(native)==88
frozen=[]
for row in inventories:
    assert row['status']==0
    data=protocol.strict_json(Path(row['report']).read_text())
    assert set(data)=={'schema','ids'} and data['schema']==1
    ids=data['ids'];assert ids==sorted(set(ids)) and len(ids) in (164,67)
    frozen.append(ids)
for row in audit_rows:
    assert json.loads((Path(row['report']).parent/'expected.json').read_text()) in frozen
assert not (OUT/'full-ci.txt').exists(),'CI-only resume must not overwrite a prior run'
state('whole85-green-restored-replayed',native_commands=len(audit_rows),observer_postcheck_correction_only=True)
env={k:v for k,v in os.environ.items() if not k.startswith('PW_MUTANT_')}
env.pop('KIOKUN_DATA',None);env.pop('KIOKUN_APP',None)
env.update(PYTHONDONTWRITEBYTECODE='1',CARGO_NET_OFFLINE='true',CARGO_BUILD_JOBS='2',RUST_TEST_THREADS='2',PORT='17201',CARGO_TARGET_DIR=str(ROOT/'target'))
assert not audit(),'pre-CI source drift'
state('full-ci-running')
started=time.monotonic()
with (OUT/'full-ci.txt').open('w') as log:
    proc=subprocess.Popen(['just','ci'],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
    state('full-ci-running',child_pid=proc.pid)
    status=proc.wait()
differences=audit()
receipt={'status':status,'seconds':round(time.monotonic()-started,3),'raw_sha256':digest(OUT/'full-ci.txt'),'differences':differences}
(OUT/'ci-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
state('complete' if status==0 and not differences else 'ci-refused',**receipt)
raise SystemExit(0 if status==0 and not differences else 1)
