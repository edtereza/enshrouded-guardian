#include <windows.h>
#include <string>
#include <cstdio>
struct ModContext;
struct Meta {std::string name,description,version,author,target;bool client,server;};
class Mod {public:bool loaded=false,active=false;virtual ~Mod()=default;virtual Meta GetMetaData()=0;virtual void Load(ModContext*)=0;virtual void Unload(ModContext*)=0;virtual void Activate(ModContext*)=0;virtual void Deactivate(ModContext*)=0;virtual void Update(ModContext*)=0;};
int wmain(int argc,wchar_t** argv){if(argc!=2)return 2;if(GetModuleHandleW(L"livemap.dll"))return 3;auto dll=LoadLibraryW(argv[1]);if(!dll){std::fprintf(stderr,"Guardian DLL load failed: %lu\n",GetLastError());return 4;}auto version=reinterpret_cast<uint32_t(*)()>(GetProcAddress(dll,"GuardianNativeBackendVersion"));auto factory=reinterpret_cast<Mod*(*)()>(GetProcAddress(dll,"CreateModInstance"));if(!version||version()!=2||!factory||!GetProcAddress(dll,"GuardianNativeManager")||!GetProcAddress(dll,"GuardianNativeDeath"))return 5;auto mod=factory();auto meta=mod->GetMetaData();if(meta.name!="Guardian"||meta.version!=GUARDIAN_VERSION||meta.client||!meta.server)return 6;if(GetModuleHandleW(L"livemap.dll"))return 7;std::puts("Guardian metadata and integrated native exports verified without Live Map.");return 0;}
