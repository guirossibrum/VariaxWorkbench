"""Bounded original PE/i386 math execution, with every OS import forbidden.

No Windows loader, Wine, driver or original DLL process is started. The PE is
mapped into Unicorn private memory at its preferred address. Unrecognized
behavior stops; no OS call is forwarded. Function identifications require
separate disassembly/call-site evidence, not merely plausible numeric results.
"""
import hashlib
import struct
from pathlib import Path
from unicorn import Uc,UC_ARCH_X86,UC_MODE_32,UC_HOOK_CODE,UC_HOOK_MEM_INVALID
from unicorn.x86_const import UC_X86_REG_EIP,UC_X86_REG_ESP,UC_X86_REG_FPCW,UC_X86_REG_FPTAG,UC_X86_REG_MXCSR
from native_oracle import ROOT


class WindowsMathOracle:
    SHA256='94c46feacf8fd5e45a9eddd4150d83705dd8895d55e5fd1140d387075e20110a'
    WRAPPER=0x20000000
    STACK=0x21000000
    RETURN=0x22000000
    TRAPS=0x23000000
    # Intrinsic wrappers identified from instruction/call-site evidence.
    ADDRESSES={'exp':0x10015d3c,'pow':0x10015dd0,'log':0x100163c0,'log10':0x10016800,'sin':0x10016950,'cos':0x10016a80,'tan':0x10016060,'sqrt':0x10016b70}
    def __init__(self,control=0x27f,dispatch="initialized-sse2",*,artifact_root=ROOT):
        if control not in (0x07f,0x27f,0x37f):raise ValueError("only the three nearest-rounding precision modes are supported")
        if dispatch not in ("initialized-sse2","image-default-x87"):raise ValueError("unknown private math dispatch mode")
        self.initial_control=control;self.dispatch=dispatch
        binary=(artifact_root/'analysis/extracted/windows-app/modules/variax/device/coefeng/L6VariaxCoefEngine.jnilib.win').read_bytes()
        if hashlib.sha256(binary).hexdigest()!=self.SHA256:
            raise ValueError('unexpected Windows coefficient PE SHA-256')
        pe=struct.unpack_from('<I',binary,0x3c)[0];assert binary[pe:pe+4]==b'PE\0\0'
        machine,sections=struct.unpack_from('<HH',binary,pe+4);assert machine==0x14c
        optional=pe+24;assert struct.unpack_from('<H',binary,optional)[0]==0x10b
        self.base=struct.unpack_from('<I',binary,optional+28)[0];size,headers=struct.unpack_from('<II',binary,optional+56)
        self.cpu=Uc(UC_ARCH_X86,UC_MODE_32);self.cpu.mem_map(self.base,(size+4095)&~4095)
        self.cpu.mem_write(self.base,binary[:headers]);table=optional+struct.unpack_from('<H',binary,pe+20)[0]
        for i in range(sections):
            _,rva,rawsize,rawoff=struct.unpack_from('<4I',binary,table+i*40+8)
            assert rawoff+rawsize<=len(binary);self.cpu.mem_write(self.base+rva,binary[rawoff:rawoff+rawsize])
        for address in [self.WRAPPER,self.STACK,self.RETURN,self.TRAPS]:self.cpu.mem_map(address,0x10000)
        self.cpu.reg_write(UC_X86_REG_FPCW,control);self.cpu.reg_write(UC_X86_REG_FPTAG,0xffff);self.cpu.reg_write(UC_X86_REG_MXCSR,0x1f80)
        self.imports={};import_rva,import_size=struct.unpack_from('<II',binary,optional+104)
        read=lambda address,n:bytes(self.cpu.mem_read(address,n))
        def cstring(address):
            out=bytearray()
            for i in range(256):
                byte=read(address+i,1)[0]
                if byte==0:return out.decode('ascii')
                out.append(byte)
            raise ValueError('unterminated import string')
        offset=0
        while True:
            lookup,_,_,name,iat=struct.unpack('<5I',read(self.base+import_rva+offset,20))
            if (lookup,name,iat)==(0,0,0):break
            dll=cstring(self.base+name);j=0
            while True:
                symbol=struct.unpack('<I',read(self.base+(lookup or iat)+j*4,4))[0]
                if symbol==0:break
                label=f'ordinal-{symbol&0xffff}' if symbol&0x80000000 else cstring(self.base+symbol+2)
                trap=self.TRAPS+len(self.imports)*16;self.imports[trap]=dll+'!'+label
                self.cpu.mem_write(self.base+iat+j*4,struct.pack('<I',trap));self.cpu.mem_write(trap,b'\xc3');j+=1
            offset+=20
            if offset>import_size:raise ValueError('import descriptors exceed directory')
        self.cpu.hook_add(UC_HOOK_CODE,self._forbidden,begin=self.TRAPS,end=self.TRAPS+0xffff)
        self.cpu.hook_add(UC_HOOK_MEM_INVALID,self._invalid)
        # Only the errno pointer getter is adapted; math and error-result code
        # execute unchanged. This avoids genuine Windows TLS/OS services.
        self.errno_address=self.WRAPPER+0x2000
        assert bytes(self.cpu.mem_read(0x10018370,6))==bytes.fromhex("e84309000085")
        self.cpu.mem_write(0x10018370,b"\xb8"+struct.pack("<I",self.errno_address)+b"\xc3")
        self.adapters={"0x10018370":"private single-thread errno pointer"}
        self.cpu.mem_map(0,0x1000);self.cpu.mem_write(0,struct.pack("<I",0xffffffff))
        if dispatch=="initialized-sse2":
            assert bytes(self.cpu.mem_read(0x1001a89c,7))==bytes.fromhex("832560bc031000")
            stack=self.STACK+0xf000;self.cpu.mem_write(stack,struct.pack("<I",self.RETURN));self.cpu.reg_write(UC_X86_REG_ESP,stack)
            self.cpu.emu_start(0x1001a89c,self.RETURN,count=1000000,timeout=2000000)
            if self.cpu.reg_read(UC_X86_REG_EIP)!=self.RETURN:raise RuntimeError("original SSE dispatcher initialization did not return")
        self.sse2_selected=struct.unpack("<I",bytes(self.cpu.mem_read(0x1003bc60,4)))[0]
    def _forbidden(self,cpu,address,size,context):raise RuntimeError('OS import forbidden: '+self.imports.get(address,hex(address)))
    def _invalid(self,cpu,access,address,size,value,context):raise RuntimeError(f'unmapped PE math access at {address:#x}, pc={cpu.reg_read(UC_X86_REG_EIP):#x}')
    def evaluate(self,name,*values,single=False):
        address=self.ADDRESSES[name]
        if len(values)!=(2 if name=='pow' else 1):raise ValueError('incorrect intrinsic argument count')
        data=self.WRAPPER+0x1000;output=data+32
        values=tuple(struct.unpack('<f',struct.pack('<f',v))[0] for v in values) if single else values
        self.cpu.mem_write(data,struct.pack('<'+'d'*len(values),*values))
        # _CIpow consumes exponent in ST0, base in ST1. Unary wrappers consume ST0.
        code=b'\xdd\x05'+struct.pack('<I',data)
        if len(values)==2:code+=b'\xdd\x05'+struct.pack('<I',data+8)
        code+=b'\xb8'+struct.pack('<I',address)+b'\xff\xd0'
        code+=(b'\xd9\x1d' if single else b'\xdd\x1d')+struct.pack('<I',output)+b'\xc3'
        self.cpu.mem_write(self.WRAPPER,code)
        self.cpu.ctl_remove_cache(self.WRAPPER,self.WRAPPER+len(code))
        stack=self.STACK+0xf000;self.cpu.mem_write(stack,struct.pack('<I',self.RETURN));self.cpu.reg_write(UC_X86_REG_ESP,stack)
        self.cpu.reg_write(UC_X86_REG_FPTAG,0xffff)
        self.cpu.emu_start(self.WRAPPER,self.RETURN,count=1000000,timeout=2000000)
        if self.cpu.reg_read(UC_X86_REG_EIP)!=self.RETURN:raise RuntimeError('bounded original math call did not return')
        return struct.unpack('<f' if single else '<d',bytes(self.cpu.mem_read(output,4 if single else 8)))[0]


if __name__=='__main__':
    oracle=WindowsMathOracle()
    for name,values in [('exp',(1.,)),('pow',(2.,3.)),('log',(2.,)),('log10',(100.,)),('sin',(1.,)),('cos',(1.,)),('tan',(.5,)),('sqrt',(4.,))]:
        try:print(name,values,oracle.evaluate(name,*values),flush=True)
        except Exception as error:print(name,'STOP:',error,flush=True)
