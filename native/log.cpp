#include "log.h"
#include <cstdio>
#include <cstdarg>
extern "C" void GuardianHardcoreLog(const char*);
namespace guardian {
void log_open(const std::wstring&,bool){}
void log(const char* format,...){char text[2048]{};va_list args;va_start(args,format);vsnprintf_s(text,sizeof text,_TRUNCATE,format,args);va_end(args);GuardianHardcoreLog(text);}
}
