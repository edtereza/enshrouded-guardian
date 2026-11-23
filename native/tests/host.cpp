#include <cstdio>
extern "C" void GuardianHardcoreLog(const char* text){std::puts(text);}

#include <cstdint>
extern "C" bool GuardianHardcoreEnabled(){return false;}
extern "C" void GuardianHardcoreBackend(uint32_t){}
extern "C" uint64_t GuardianHardcoreDeath(uint32_t,uint64_t){return 0;}
extern "C" uint64_t GuardianHardcorePoll(uint32_t,uint64_t){return 0;}
extern "C" bool GuardianHardcoreComplete(uint32_t,uint64_t,uint64_t){return false;}
