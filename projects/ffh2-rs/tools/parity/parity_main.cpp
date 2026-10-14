// Parity harness: compiles the original DLL's CvRandom.cpp and CvFractal.cpp
// and prints reference outputs that the Rust port is tested against
// (crates/ffh-core/tests/fixtures/*.txt).
#include "CvGameCoreDLL.h"
#include "CvRandom.h"
#include "CvFractal.h"
#include <cstring>

CvDLLStub* gDLL = new CvDLLStub();
CvGlobalsStub GC;

int main(int argc, char** argv) {
    if (argc > 1 && strcmp(argv[1], "random") == 0) {
        unsigned long seeds[] = {0, 1, 12345, 4294967295UL, 2654435761UL};
        unsigned short nums[] = {0, 1, 2, 7, 100, 1000, 65535};
        for (int s = 0; s < 5; s++) {
            CvRandom r; r.init(seeds[s]);
            printf("seed %lu:", seeds[s]);
            for (int i = 0; i < 40; i++) printf(" %u", r.get(nums[i % 7], NULL));
            float fv = r.getFloat();
            unsigned long endSeed = r.getSeed();
            printf(" f=%.6f end=%lu\n", fv, endSeed);
        }
        return 0;
    }
    // fractal: xs ys grain flags fracXExp fracYExp seed [hinted]
    struct Cfg { int xs, ys, grain, flags, xe, ye; unsigned long seed; bool hints; bool rifts; };
    Cfg cfgs[] = {
        {64, 40, 2, 0, 7, 6, 1, false, false},
        {80, 52, 3, CvFractal::FRAC_WRAP_X, 7, 6, 42, false, false},
        {80, 52, 4, CvFractal::FRAC_WRAP_X | CvFractal::FRAC_POLAR, 7, 6, 7, false, false},
        {104, 64, 1, CvFractal::FRAC_WRAP_X | CvFractal::FRAC_PERCENT, 7, 6, 99, false, false},
        {52, 32, 3, CvFractal::FRAC_WRAP_X | CvFractal::FRAC_WRAP_Y, 6, 5, 2024, false, false},
        {64, 40, 3, CvFractal::FRAC_WRAP_X, 7, 6, 5, true, false},
        {80, 52, 3, CvFractal::FRAC_WRAP_X, 7, 6, 77, false, true},
        {80, 52, 2, CvFractal::FRAC_WRAP_X | CvFractal::FRAC_INVERT_HEIGHTS, 7, 6, 13, false, false},
    };
    for (int c = 0; c < 8; c++) {
        Cfg& k = cfgs[c];
        CvRandom r; r.init(k.seed);
        CvFractal f;
        CvFractal rifts;
        if (k.rifts) {
            rifts.fracInit(k.xs, k.ys, 1, r, k.flags, NULL, k.xe, k.ye);
        }
        if (k.hints) {
            int minExp = std::min(k.xe, k.ye);
            int smooth = range(minExp - k.grain, 0, minExp);
            int w = (1 << (k.xe - smooth)) + ((k.flags & CvFractal::FRAC_WRAP_X) ? 0 : 1);
            int h = (1 << (k.ye - smooth)) + ((k.flags & CvFractal::FRAC_WRAP_Y) ? 0 : 1);
            byte* hints = new byte[w * h];
            for (int i = 0; i < w * h; i++) hints[i] = (byte)((i * 37 + 11) % 256);
            f.fracInitHinted(k.xs, k.ys, k.grain, r, hints, w * h, k.flags, NULL, k.xe, k.ye);
            delete[] hints;
        } else {
            f.fracInit(k.xs, k.ys, k.grain, r, k.flags, k.rifts ? &rifts : NULL, k.xe, k.ye);
        }
        printf("cfg %d xs=%d ys=%d grain=%d flags=%d xe=%d ye=%d seed=%lu hints=%d rifts=%d\n", c, k.xs, k.ys, k.grain, k.flags, k.xe, k.ye, k.seed, k.hints, k.rifts);
        printf("pct:");
        for (int p = 0; p <= 100; p += 5) printf(" %d", f.getHeightFromPercent(p));
        printf("\n");
        for (int y = 0; y < k.ys; y++) {
            for (int x = 0; x < k.xs; x++) printf("%d ", f.getHeight(x, y));
            printf("\n");
        }
        printf("rng_end=%lu\n", r.getSeed());
    }
    return 0;
}
