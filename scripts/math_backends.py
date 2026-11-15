"""Explicit alternate math adapters for offline reference sensitivity only."""
import ctypes
import ctypes.util
import hashlib
import math
import struct
from pathlib import Path

FUNCTIONS={
    'sin':(0,1,False),'sinf':(0,1,True),'cos':(1,1,False),'cosf':(1,1,True),
    'tan':(2,1,False),'tanf':(2,1,True),'exp':(3,1,False),'expf':(3,1,True),
    'log':(4,1,False),'log10f':(5,1,True),'pow':(6,2,False),'powf':(6,2,True),'atan2f':(7,2,True),
}


def bits(value,single):
    return struct.pack('<f' if single else '<d',value).hex()


def same_number(a,b):
    return a==b or (math.isnan(a) and math.isnan(b))


class AdapterFunction:
    # _dispatch sets these attributes as it does on a ctypes function. Each
    # implementation fixes the real C signature before receiving arguments.
    argtypes=None
    restype=None
    def __init__(self,call):self.call=call
    def __call__(self,*values):return self.call(*values)


class AppleSourceMath:
    def __init__(self,path):
        self.library=ctypes.CDLL(str(path));self.methods={}
        for name,(_,arity,single) in FUNCTIONS.items():
            method=getattr(self.library,'apple92_'+name if name in ('pow','powf') else '_'+name)
            scalar=ctypes.c_float if single else ctypes.c_double
            method.argtypes=[scalar]*arity;method.restype=scalar;self.methods[name]=method
    def __getattr__(self,name):return self.methods[name]


class MPFRMath:
    def __init__(self,path,precision=256):
        if precision not in (256,512):raise ValueError('audit precision must be 256 or 512')
        self.precision=precision;self.library=ctypes.CDLL(str(path));method=self.library.audit_math
        method.argtypes=[ctypes.c_int,ctypes.c_double,ctypes.c_double,ctypes.c_int,ctypes.c_uint];method.restype=ctypes.c_double
        self.methods={}
        for name,(operation,arity,single) in FUNCTIONS.items():
            def evaluate(*values,operation=operation,arity=arity,single=single):
                if len(values)!=arity:raise ValueError('incorrect math arity')
                if single:values=tuple(ctypes.c_float(v).value for v in values)
                return method(operation,values[0],values[1] if arity==2 else 0.,int(single),precision)
            self.methods[name]=AdapterFunction(evaluate)
    def __getattr__(self,name):return self.methods[name]
    def environment(self):
        result={}
        for field,method_name,ctype in [('mpfr','audit_mpfr_version',ctypes.c_char_p),('glibc','audit_glibc_version',ctypes.c_char_p),('host_x87_control','audit_x87_control',ctypes.c_uint),('host_mxcsr','audit_mxcsr',ctypes.c_uint),('host_rounding_mode','audit_rounding_mode',ctypes.c_int)]:
            method=getattr(self.library,method_name);method.argtypes=[];method.restype=ctype;value=method();result[field]=value.decode() if isinstance(value,bytes) else value
        return result


class TraceMath:
    def __init__(self):
        self.library=ctypes.CDLL(ctypes.util.find_library('m'));self.records={};self.methods={};self.limit=500000
        for name,(_,arity,single) in FUNCTIONS.items():
            method=getattr(self.library,name);scalar=ctypes.c_float if single else ctypes.c_double;method.argtypes=[scalar]*arity;method.restype=scalar
            def evaluate(*values,method=method,name=name,single=single):
                result=method(*values);key=(name,tuple(bits(v,single) for v in values));encoded=bits(result,single)
                if key in self.records:
                    record=self.records[key]
                    if record['result_bits']!=encoded:raise RuntimeError('non-repeatable host libm output for identical input bits')
                    record['calls']+=1
                else:
                    if len(self.records)>=self.limit:raise RuntimeError('math argument trace exceeded its explicit bound')
                    self.records[key]={'function':name,'input_bits':list(key[1]),'result_bits':encoded,'calls':1}
                return result
            self.methods[name]=AdapterFunction(evaluate)
    def __getattr__(self,name):return self.methods[name]


def mapped_libraries():
    # CPU runtime provenance only. No USB/MIDI/audio descriptors are inspected.
    found={}
    for line in Path('/proc/self/maps').read_text().splitlines():
        fields=line.split()
        if len(fields)<6:continue
        path=Path(fields[-1])
        if path.name in ['libm.so.6','libc.so.6'] or path.name.startswith(('libmpfr.so.','libgmp.so.','libunicorn.so.')):
            if path.is_file():found[str(path)]=hashlib.sha256(path.read_bytes()).hexdigest()
    return found


class WindowsIntrinsicMath:
    """Original embedded Windows CRT intrinsics, with explicit width adaptation.

    Float aliases mean: take the same float32 input as the Mac import and round
    the intrinsic's result directly to float32. This isolates math routines; it
    does not reproduce the Windows coefficient compiler's surrounding arithmetic.
    """
    def __init__(self,control=0x27f,dispatch='initialized-sse2'):
        from windows_math_oracle import WindowsMathOracle
        self.oracle=WindowsMathOracle(control,dispatch);self.methods={};self.cache={};self.calls=0;self.errors=[]
        for name,(_,arity,single) in FUNCTIONS.items():
            base=name[:-1] if single else name
            if base not in self.oracle.ADDRESSES:continue
            def evaluate(*values,base=base,single=single,arity=arity):
                if len(values)!=arity:raise ValueError('incorrect original intrinsic arity')
                self.calls+=1
                if single:values=tuple(ctypes.c_float(v).value for v in values)
                key=(base,single,tuple(bits(v,False) for v in values))
                if key not in self.cache:
                    if len(self.cache)>=500000:raise RuntimeError('original intrinsic cache exceeded bound')
                    self.cache[key]=self.oracle.evaluate(base,*values,single=single)
                return self.cache[key]
            self.methods[name]=AdapterFunction(evaluate)
    def __getattr__(self,name):
        if name not in self.methods:raise RuntimeError('unrecovered original Windows intrinsic: '+name)
        return self.methods[name]


class RustSoftwareMath:
    """Pinned force-soft-floats helper; optional research, never a fallback."""
    def __init__(self,path,*,widen_float=False):
        self.library=ctypes.CDLL(str(path));method=self.library.audit_rust_math
        method.argtypes=[ctypes.c_int,ctypes.c_double,ctypes.c_double,ctypes.c_int];method.restype=ctypes.c_double
        self.methods={}
        for name,(operation,arity,single) in FUNCTIONS.items():
            def evaluate(*values,operation=operation,arity=arity,single=single):
                if len(values)!=arity:raise ValueError('incorrect math arity')
                if single and widen_float:
                    values=tuple(ctypes.c_float(v).value for v in values)
                    return ctypes.c_float(method(operation,values[0],values[1] if arity==2 else 0.,0)).value
                return method(operation,values[0],values[1] if arity==2 else 0.,int(single))
            self.methods[name]=AdapterFunction(evaluate)
    def __getattr__(self,name):return self.methods[name]
