/* Offline high-precision comparison helper. No files, devices or networking. */
#include <mpfr.h>
#include <fenv.h>
#include <stdint.h>
#include <math.h>
#include <gnu/libc-version.h>

const char *audit_mpfr_version(void) { return mpfr_get_version(); }
const char *audit_glibc_version(void) { return gnu_get_libc_version(); }
unsigned audit_x87_control(void) { unsigned short cw; __asm__ volatile("fnstcw %0" : "=m"(cw)); return cw; }
unsigned audit_mxcsr(void) { unsigned csr; __asm__ volatile("stmxcsr %0" : "=m"(csr)); return csr; }
int audit_rounding_mode(void) { return fegetround(); }

/* MPFR rounds the mathematical operation at the requested working precision,
 * then converts directly to float/double with round-to-nearest, ties-to-even.
 * The caller compares multiple working precisions; this is not a macOS binary.
 */
double audit_math(int operation, double x, double y, int single, unsigned precision) {
    if (precision < 128 || precision > 4096) return NAN;
    mpfr_t a,b,r;mpfr_inits2(precision,a,b,r,(mpfr_ptr)0);
    mpfr_set_d(a,x,MPFR_RNDN);mpfr_set_d(b,y,MPFR_RNDN);
    switch(operation) {
        case 0: mpfr_sin(r,a,MPFR_RNDN);break;
        case 1: mpfr_cos(r,a,MPFR_RNDN);break;
        case 2: mpfr_tan(r,a,MPFR_RNDN);break;
        case 3: mpfr_exp(r,a,MPFR_RNDN);break;
        case 4: mpfr_log(r,a,MPFR_RNDN);break;
        case 5: mpfr_log10(r,a,MPFR_RNDN);break;
        case 6: mpfr_pow(r,a,b,MPFR_RNDN);break;
        case 7: mpfr_atan2(r,a,b,MPFR_RNDN);break;
        default: mpfr_clears(a,b,r,(mpfr_ptr)0);return NAN;
    }
    double output = single ? (double)mpfr_get_flt(r,MPFR_RNDN) : mpfr_get_d(r,MPFR_RNDN);
    mpfr_clears(a,b,r,(mpfr_ptr)0);return output;
}
