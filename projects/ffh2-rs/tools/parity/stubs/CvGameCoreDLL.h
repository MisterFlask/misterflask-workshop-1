// Minimal stand-in for the DLL's precompiled header, enough to compile
// self-contained DLL sources (CvRandom.cpp, CvFractal.cpp) on Linux for parity tests.
#pragma once
#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cmath>
#include <cassert>
typedef unsigned char byte;
typedef unsigned char BYTE;
typedef char TCHAR;
typedef unsigned int uint;
#define DllExport
#define MAX_UNSIGNED_SHORT (65535)
#define MAX_INT 0x7FFFFFFF
#define SAFE_DELETE_ARRAY(p) { if (p) { delete[] (p); (p) = NULL; } }
#define FAssert(x) { assert(x); }
#define FAssertMsg(x, m) { assert(x); }
#define PROFILE(x)
class FDataStreamBase { public: template<class T> void Read(T*) {} template<class T> void Write(T) {} };
struct CvDLLStub { void callUpdater() {} void messageControlLog(const char*) {} };
extern CvDLLStub* gDLL;
struct CvGameStub { int getTurnSlice() const { return 0; } };
struct CvGlobalsStub { bool getLogging() const { return false; } bool getRandLogging() const { return false; } CvGameStub& getGameINLINE() { static CvGameStub g; return g; } };
extern CvGlobalsStub GC;
template <class T> inline T range(T iNum, T iLow, T iHigh) { return (iNum < iLow) ? iLow : ((iNum > iHigh) ? iHigh : iNum); }
