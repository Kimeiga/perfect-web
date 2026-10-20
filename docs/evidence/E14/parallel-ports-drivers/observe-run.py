import sys, pathlib, json, subprocess, runpy
ROOT=pathlib.Path('/home/haki/code/pw-wt/parallel-instrument')
OUT=pathlib.Path('/tmp/parallel-ports-v1')
original_wait=subprocess.Popen.wait
seen={}
errors=[]
def record_error(error):
    errors.append(type(error).__name__+': '+str(error))
def observed_wait(self,*args,**kwargs):
    status=original_wait(self,*args,**kwargs)
    try:
        argv=self.args
        if isinstance(argv,(tuple,list)) and '--report' in argv:
            report=str(argv[argv.index('--report')+1])
            key=(self.pid,report)
            if key in seen:
                if seen[key]!=status: raise ValueError('contradictory native wait status')
            else:
                seen[key]=status
                item={'pid':self.pid,'argv':list(argv),'status':status,'report':report,
                      'inventory':'--inventory' in argv}
                with (OUT/'native-waits.jsonl').open('a') as log: log.write(json.dumps(item)+'\n')
    except BaseException as error:
        record_error(error)
    return status
subprocess.Popen.wait=observed_wait
sys.path.insert(0,str(ROOT/'scripts'))
sys.argv=[str(ROOT/'scripts/mutation_slots_mutations.py')]
try:
    runpy.run_path(sys.argv[0],run_name='__main__')
finally:
    subprocess.Popen.wait=original_wait
    (OUT/'observer-final.json').write_text(json.dumps({'errors':errors,'native_records':len(seen)},indent=2)+'\n')
