"""Independent synthetic resource encoder for valid all-block differential inputs."""
import random
import struct
from native_oracle import ROOT


def make_body(seed):
    rng=random.Random(60039000+seed)
    def floats(values):return struct.pack('>'+'f'*len(values),*values)
    def integers(values):return struct.pack('>'+'I'*len(values),*values)
    def choices(values,count):return [rng.choice(values) for _ in range(count)]
    def flag():return bytes([rng.randrange(2)])
    def eq(n):
        return integers(choices([0,1,2,3,4,5],n))+floats(choices([20.,110.,1200.,6000.],n)+choices([.3,.707,1.5,3.],n)+choices([-12.,0.,12.],n)+choices([0.,.5,1.,1.25],n))
    header=bytearray((ROOT/'tests/fixtures/native-body-definition.bin').read_bytes()[:101]);header[97:101]=integers([0]);variants=[]
    for variant in range(6):
        data=bytearray()
        data+=flag()+bytes(choices([0,1],12))+floats(choices([20.,110.,440.,1200.,6000.],12)+choices([.3,.707,2.,10.],12)+choices([-24.,-6.,0.,6.],12)+[rng.choice([0.,100.,1000.]),rng.choice([0.,800.,8000.]),rng.choice([-12.,0.,6.]),rng.uniform(-1.,1.),rng.uniform(-1.,1.),rng.uniform(-1.,1.)])
        data+=flag()+floats([rng.choice([0.,20.,80.,1000.])])+flag()
        data+=flag()
        for _ in range(6):data+=floats([rng.uniform(-1.,2.)])+eq(6)
        data+=flag()
        for _ in range(6):data+=eq(2)
        data+=flag()
        for _ in range(6):data+=floats([rng.choice([-12.,0.,6.]),rng.choice([0.,.001,.1]),rng.choice([0.,.01,1.])])+bytes(choices([0,1],6))+floats(choices([20.,110.,440.,1200.],6)+choices([.3,.707,2.],6)+choices([-12.,0.,6.],6))
        data+=flag()+floats(choices([0.,.001,.1,1.],6)+choices([0.,.01,.2,2.],6)+choices([0.,.001,.125],6))
        data+=floats([rng.uniform(-1.,1.) for _ in range(6)])
        data+=flag()+floats([rng.uniform(0.,1.) for _ in range(9)])+integers([rng.randrange(4)])+floats([rng.uniform(-12.,6.)])
        data+=flag()+floats(choices([0.,1.,10.,50.],4)+[rng.uniform(-1.,1.) for _ in range(9)]+choices([0.,100.,1000.,3000.],9)+[rng.choice([0.,10.,50.])])+integers([rng.randrange(4),rng.randrange(4)])
        data+=flag()+integers([rng.choice([0,1,6,12])])+floats([rng.uniform(0.,1.)]+choices([0.,55.,110.,220.],12)+[rng.uniform(-1.,1.) for _ in range(12)])
        data+=flag()+floats(choices([0.,.001,.1,1.],6)+choices([0.,40.,440.,1200.],6)+choices([0.,20.,80.,440.],6)+choices([0.,.1,.5,1.,6.46],6)+choices([0.,.5,1.],6))
        data+=flag()+floats(choices([-12.,-2.5,0.,7.,12.],6)+choices([0.,.001,.5,5.],6)+choices([0.,.5,1.],6)+choices([0.,.5,1.],6))
        data+=flag()+floats(choices([0.,20.,1000.,10000.],9)+choices([0.,.125,1.,2.],9)+choices([.1,.707,1.,3.],9)+choices([0.,.5,1.,2.],9)+choices([0.,110.,1200.,6000.],9)+choices([.1,.707,1.,3.],9)+choices([0.,.5,1.,2.],9)+[rng.choice([.125,1.,2.]),rng.choice([0.,110.,1000.])])
        data+=flag()+floats([rng.choice([24.75,25.,25.5])])+integers([rng.randrange(5)])+integers(choices([0,1,2,3],6))
        for _ in range(4):data+=floats(choices([-4.,-0.,0.,.125,1.,2.,4.,8.,12.],6))+bytes(choices([0,1],6))
        optional=(seed+variant)%8
        data+=bytes([optional&1])+floats([rng.choice([110.,440.,1000.,6000.]),rng.choice([.3,.707,2.]),rng.choice([0.,.001,.1])]+[rng.uniform(-2.,2.) for _ in range(9)])
        data+=bytes([(optional>>1)&1])+floats([rng.choice([0.,.001,.1]),rng.choice([0.,.02,.2])]+[rng.uniform(-1.,2.) for _ in range(9)])
        data+=bytes([(optional>>2)&1])+floats([rng.uniform(-1.,1.) for _ in range(9)]+[rng.uniform(-5.,30.) for _ in range(9)]+[rng.uniform(-1.,1.) for _ in range(9)])
        assert len(data)==2897,len(data);variants.append(data)
    payload=bytes(header)+b''.join(variants);assert len(payload)==17483
    return payload
