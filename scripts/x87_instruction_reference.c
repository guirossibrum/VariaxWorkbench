/* Research-only native CPU instruction comparison; no vendor code or libm. */
#include <stdint.h>

/* Inputs must be finite and within +/- 2^20. Those bounds keep this helper
 * away from x87 range-reduction fallback; callers record rejected scenarios. */
double x87_reference(int operation, double input, int single) {
    if (operation < 0 || operation > 2 || !(input > -1048576.0 && input < 1048576.0))
        return __builtin_nan("");
    unsigned short previous, nearest = 0x37f;
    __asm__ volatile("fnstcw %0" : "=m"(previous));
    __asm__ volatile("fldcw %0" : : "m"(nearest));
    double result;
    if (single) {
        float value = (float)input, output;
        if (operation == 0) __asm__ volatile("flds %1; fsin; fstps %0" : "=m"(output) : "m"(value) : "st");
        else if (operation == 1) __asm__ volatile("flds %1; fcos; fstps %0" : "=m"(output) : "m"(value) : "st");
        else __asm__ volatile("flds %1; fptan; fstp %%st(0); fstps %0" : "=m"(output) : "m"(value) : "st");
        result = output;
    } else {
        if (operation == 0) __asm__ volatile("fldl %1; fsin; fstpl %0" : "=m"(result) : "m"(input) : "st");
        else if (operation == 1) __asm__ volatile("fldl %1; fcos; fstpl %0" : "=m"(result) : "m"(input) : "st");
        else __asm__ volatile("fldl %1; fptan; fstp %%st(0); fstpl %0" : "=m"(result) : "m"(input) : "st");
    }
    __asm__ volatile("fldcw %0" : : "m"(previous));
    return result;
}
