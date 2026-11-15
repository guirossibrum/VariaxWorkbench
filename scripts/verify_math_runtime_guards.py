"""Offline guard, wrapper-cache and reference-width regressions."""
import math
import struct
import unittest
from pathlib import Path
from coefficient_oracle import CoefficientOracle,ROOT
from windows_math_oracle import WindowsMathOracle
from math_backends import MPFRMath,AppleSourceMath,FUNCTIONS


class RuntimeGuards(unittest.TestCase):
    def test_alternate_math_cannot_reuse_a_canonical_namespace(self):
        for kwargs in [{'runtime_version':'other'},{'math_backend':object()},{'math_backend':object(),'runtime_version':CoefficientOracle.RUNTIME_VERSION}]:
            with self.assertRaises(ValueError):CoefficientOracle(**kwargs)
    def test_original_windows_wrapper_switches_functions_and_result_widths(self):
        oracle=WindowsMathOracle()
        for _ in range(3):
            for name,args,expected in [('sin',(0.,),0.),('cos',(0.,),1.),('exp',(0.,),1.),('pow',(2.,3.),8.),('sqrt',(4.,),2.),('log',(1.,),0.)]:
                self.assertEqual(oracle.evaluate(name,*args),expected)
                self.assertEqual(oracle.evaluate(name,*args,single=True),expected)
        self.assertEqual(oracle.sse2_selected,1)
    def test_private_errno_handles_domains_without_os_calls(self):
        oracle=WindowsMathOracle();self.assertEqual(oracle.evaluate('log',0.),-math.inf)
        self.assertEqual(struct.unpack('<I',bytes(oracle.cpu.mem_read(oracle.errno_address,4)))[0],34)
        self.assertTrue(math.isnan(oracle.evaluate('log',-1.)))
        self.assertEqual(struct.unpack('<I',bytes(oracle.cpu.mem_read(oracle.errno_address,4)))[0],33)
        self.assertEqual(oracle.evaluate('exp',-math.inf),0.)
    def test_os_imports_and_unreviewed_modes_remain_forbidden(self):
        oracle=WindowsMathOracle()
        with self.assertRaisesRegex(RuntimeError,'OS import forbidden'):
            oracle.cpu.emu_start(oracle.TRAPS,oracle.RETURN,count=1000,timeout=100000)
        for kwargs in [{'control':0},{'dispatch':'anything'}]:
            with self.assertRaises(ValueError):WindowsMathOracle(**kwargs)
    def test_mpfr_reference_preserves_widths_and_environment(self):
        ref=MPFRMath((ROOT/'.tools/math-reference/libmpfr-reference.so').resolve())
        higher=MPFRMath((ROOT/'.tools/math-reference/libmpfr-reference.so').resolve(),512)
        for name in FUNCTIONS:
            args=(2.,3.) if name in ('pow','powf','atan2f') else (.5,)
            self.assertEqual(getattr(ref,name)(*args),getattr(higher,name)(*args))
        self.assertEqual(ref.environment()['host_rounding_mode'],0)
        self.assertEqual(ref.exp(-math.inf),0.)
        with self.assertRaises(ValueError):MPFRMath(Path('unused'),100)


if __name__=='__main__':unittest.main()
