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
assert not audit(),'prelaunch source drift'
env={k:v for k,v in os.environ.items() if not k.startswith('PW_MUTANT_')}
env.pop('KIOKUN_DATA',None);env.pop('KIOKUN_APP',None)
env.update(PYTHONDONTWRITEBYTECODE='1',CARGO_NET_OFFLINE='true',CARGO_BUILD_JOBS='2',RUST_TEST_THREADS='2',PORT='17201',CARGO_TARGET_DIR=str(ROOT/'target'))
state('whole85-running')
started=time.monotonic()
with (OUT/'whole85.txt').open('w') as log:
    proc=subprocess.Popen(['python3','-B',str(OUT/'observe-run.py')],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
    state('whole85-running',child_pid=proc.pid)
    status=proc.wait()
differences=audit()
receipt={'status':status,'seconds':round(time.monotonic()-started,3),'raw_sha256':digest(OUT/'whole85.txt'),'differences':differences}
(OUT/'whole-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
state('whole85-ended',**receipt)
assert status==0 and not differences,'whole85 did not pass and restore'
observer=json.loads((OUT/'observer-final.json').read_text());assert not observer['errors'],observer
rows=[json.loads(line) for line in (OUT/'native-waits.jsonl').read_text().splitlines()]
tests=[r for r in rows if not r['inventory']]
assert len(rows)==88 and len(tests)==86,(len(rows),len(tests))
spec=importlib.util.spec_from_file_location('frozen_ports_protocol',OUT/'unittest_protocol.py');protocol=importlib.util.module_from_spec(spec);spec.loader.exec_module(protocol)
proof=[]
for i,row in enumerate(tests):
    folder=Path(row['report']).parent
    ids=json.loads((folder/'expected.json').read_text())
    assert len(ids) in (164,67),len(ids)
    result=protocol.validate_result(row['status'],Path(row['report']).read_text(),ids)
    assert result.valid,(i,result.reason)
    assert row['status']==(0 if i==0 else 1),(i,row['status'])
    assert (not result.failed) if i==0 else bool(result.failed),(i,result)
    proof.append({'ordinal':i,'pid':row['pid'],'status':row['status'],'methods':len(ids),'report':row['report'],'report_sha256':digest(Path(row['report'])),'output_sha256':digest(folder/'output.txt'),'passed':len(result.passed),'assertion_failed':list(result.assertion_failed),'body_errors':list(result.body_errors)})
(OUT/'independent-report-audit.json').write_text(json.dumps(proof,indent=2)+'\n')
assert not re.search(r'^memory bound:.*(?:stopped|perhaps killed)',(OUT/'whole85.txt').read_text(),re.M)
state('whole85-green-restored-replayed',native_commands=len(tests),assertion_controls=sum(bool(x['assertion_failed']) for x in proof[1:]),body_error_controls=sum(bool(x['body_errors']) for x in proof[1:]))
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
