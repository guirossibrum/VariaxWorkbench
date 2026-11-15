"""Synthetic FFI-width/special-value checks for optional research helpers."""
import math
import unittest
from coefficient_oracle import ROOT
from math_backends import RustSoftwareMath,bits


class RustMathReference(unittest.TestCase):
    def setUp(self):
        directory=ROOT/'.tools/rust-math-reference/target'
        self.backends=[RustSoftwareMath((directory/p/'libvariax_math_research.so').resolve(),widen_float=widen) for widen in [False,True] for p in ['debug','release']]
    def test_ffi_dispatch_and_input_width_are_explicit(self):
        for ref in self.backends:
            for name,args,expected in [('sin',(0.,),0.),('cos',(0.,),1.),('exp',(0.,),1.),('log',(1.,),0.),('log10f',(100.,),2.),('pow',(2.,3.),8.),('atan2f',(0.,1.),0.)]:
                self.assertEqual(getattr(ref,name)(*args),expected)
            self.assertEqual(ref.pow(16777217.,1.),16777217.)
            self.assertEqual(ref.powf(16777217.,1.),16777216.)
            with self.assertRaises(ValueError):ref.pow(2.)
    def test_domains_infinities_and_signed_zero(self):
        for ref in self.backends:
            self.assertEqual(ref.exp(-math.inf),0.)
            self.assertEqual(ref.log(0.),-math.inf)
            self.assertTrue(math.isnan(ref.log(-1.)))
            self.assertTrue(math.isnan(ref.pow(-1.,.5)))
            self.assertEqual(bits(ref.sin(-0.),False),bits(-0.,False))
            self.assertEqual(bits(ref.sinf(-0.),True),bits(-0.,True))
    def test_synthetic_subnormal_and_boundary_profile_repeatability(self):
        for name,single in [('sin',False),('sinf',True),('cos',False),('exp',False),('expf',True),('tan',False),('tanf',True)]:
            for value in [0.,-0.,2.**-149,-2.**-149,2.**-126,1.,-1.,3.141592653589793]:
                for debug,release in zip(self.backends[::2],self.backends[1::2]):
                    self.assertEqual(bits(getattr(debug,name)(value),single),bits(getattr(release,name)(value),single))


if __name__=='__main__':unittest.main()
