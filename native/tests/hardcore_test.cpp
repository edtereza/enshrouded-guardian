#include "hardcore_native.h"
#include <cstdio>
#include <vector>
#include <stdexcept>
#include <string>
namespace guardian::hardcore { std::vector<uint32_t> test_inventory_ids(uint8_t*,uint32_t,void(*)(uint64_t*,uint8_t*,uint32_t,uint32_t)); bool test_is_tombstone(uint8_t*,uint32_t); void test_teleport_components(uint8_t*,void(*)(uint64_t*,uint8_t*,uint32_t,uint32_t)); bool test_native_diagnostics(); uint32_t test_start(uint8_t*,void(*)(uint64_t*,uint8_t*,uint32_t,uint32_t)); }
using namespace guardian::hardcore::native;
template<class T>void put(uint8_t* p,size_t off,T value){std::memcpy(p+off,&value,sizeof value);}
Transform spawn_transform{};
LastTransform last_transform{};
uint8_t teleport_count[16]{};
bool wrong_last_transform=false;
void fake_component(uint64_t* out,uint8_t*,uint32_t type,uint32_t entity){out[0]=0;out[1]=0;if(type==hash("keen::ecs::CurrentTransform")&&(entity==91||entity==92)){out[0]=reinterpret_cast<uintptr_t>(&spawn_transform);out[1]=56;}}
void fake_teleport_components(uint64_t* out,uint8_t* world,uint32_t type,uint32_t entity){
    fake_component(out,world,type,entity);
    if(type==hash("keen::ecs::LastTransform")){out[0]=reinterpret_cast<uintptr_t>(&last_transform);out[1]=wrong_last_transform?56:sizeof(LastTransform);}
    if(type==hash("keen::ecs::TeleportCount")){out[0]=reinterpret_cast<uintptr_t>(teleport_count);out[1]=sizeof teleport_count;}
}
Setup root_setup{},nested_setup{};
void fake_inventory_setup(uint64_t* out,uint8_t*,uint32_t type,uint32_t entity){out[0]=0;out[1]=0;if(type==hash("keen::ecs::InventorySetup")&&(entity==10||entity==11)){out[0]=reinterpret_cast<uintptr_t>(entity==10?&root_setup:&nested_setup);out[1]=sizeof(Setup);}}
int main(){
#define CHECK(x) if(!(x)){std::fprintf(stderr,"Failed line %d: %s\n",__LINE__,#x);return 1;}
    CHECK(guardian::hardcore::test_native_diagnostics());
    guardian::hardcore::test_teleport_components(nullptr,fake_teleport_components);
    wrong_last_transform=true;
    bool rejected=false;
    try { guardian::hardcore::test_teleport_components(nullptr,fake_teleport_components); }
    catch(const std::runtime_error& e){rejected=std::string(e.what())=="component layout changed: LastTransform";}
    CHECK(rejected);
    wrong_last_transform=false;
    CHECK(exact_session(65,100,65,100));
    CHECK(!exact_session(65,100,129,100)); // same slot, another connection
    CHECK(!exact_session(65,100,65,101)); // another account
    CHECK(!exact_session(65,0,65,0));
    CHECK(progress_proof(1,0,0,true,true));
    CHECK(!progress_proof(1,0,1,true,true));
    CHECK(!progress_proof(1,0,0,false,true));
    CHECK(!progress_proof(1,0,0,true,false));
    CHECK(!progress_proof(2,0,0,true,true));
    Transform a{},b{};a.position[0]=int64_t{25}<<32;CHECK(near_start(a,b));a.position[0]+=1LL<<32;CHECK(!near_start(a,b));
    CHECK(hash("keen::ecs::Teleport")==0x23384d40);
    CHECK(hash("keen::ecs::PlayerSpawnPoint")==0x08060e71);
    // Actual scanner over synthetic ECS memory: the outer world starts with a state,
    // while its entity storage owns the registry and archetype table.
    std::vector<uint8_t> world(storage+0x150),registry(0x20),types(0x100*0x100),arch(0xb00),chunk(0x400);
    uint8_t* arches[]={reinterpret_cast<uint8_t*>(1),arch.data(),reinterpret_cast<uint8_t*>(0xffffffffffffffffULL)};world[0]=2;
    put(world.data(),storage,registry.data());put(registry.data(),8,types.data());put<uint64_t>(registry.data(),16,0x100);
    put<uint32_t>(types.data(),5*0x100+0x20,hash("keen::ecs::PlayerSpawnPoint"));
    put(world.data(),storage+0x130,arches);put<uint32_t>(world.data(),storage+0x140,3);
    put<uint64_t>(arch.data(),0,uint64_t{1}<<5);put(arch.data(),0x50,chunk.data());put<uint16_t>(arch.data(),0x84+10,0x100);put<uint16_t>(arch.data(),0xa84+10,204);
    put<uint32_t>(chunk.data(),0x10,1);put<uint16_t>(chunk.data(),0x14,0x80);put<uint32_t>(chunk.data(),0x80,91);
    CHECK(guardian::hardcore::test_start(world.data(),fake_component)==91);
    put<uint32_t>(chunk.data(),0x10,2);put<uint32_t>(chunk.data(),0x90,92);
    CHECK(guardian::hardcore::test_start(world.data(),fake_component)==0); // ambiguous start
    chunk[0x100+204]=1; // second is a save point, not the initial location
    CHECK(guardian::hardcore::test_start(world.data(),fake_component)==91);
    chunk[0x100]=2;
    CHECK(guardian::hardcore::test_start(world.data(),fake_component)==0);
    // A tombstone is identified by its actual entity template, never by proximity.
    std::vector<uint8_t> tomb_world(storage+0x1a0),record(0x38),tomb_chunk(0x100),info(0x20);
    uint64_t occupied=1;uint32_t key=123;uint8_t* value=record.data();const char* name="Base_LootContainer_Player_LootCorpse_Tombstone";
    auto map=tomb_world.data()+storage+0x148;put(map,8,&occupied);put<uint64_t>(map,0x10,1);put(map,0x28,&key);put(map,0x40,&value);
    put(record.data(),0x20,tomb_chunk.data());put<uint32_t>(record.data(),0x30,0);put<uint32_t>(tomb_chunk.data(),0x10,1);put<uint16_t>(tomb_chunk.data(),0x18,0x80);put(tomb_chunk.data(),0x80,info.data());put(info.data(),0x10,name);put<uint64_t>(info.data(),0x18,std::strlen(name));
    CHECK(guardian::hardcore::test_is_tombstone(tomb_world.data(),123));
    CHECK(!guardian::hardcore::test_is_tombstone(tomb_world.data(),124));
    occupied=0;CHECK(!guardian::hardcore::test_is_tombstone(tomb_world.data(),123));
    occupied=1;const char* chest="LootChest_Random";put(info.data(),0x10,chest);put<uint64_t>(info.data(),0x18,std::strlen(chest));CHECK(!guardian::hardcore::test_is_tombstone(tomb_world.data(),123));
    root_setup.initialized=nested_setup.initialized=1;root_setup.links[0]=11;root_setup.categories[0]=5;root_setup.links[1]=88;root_setup.categories[1]=4;nested_setup.links[0]=88;nested_setup.categories[0]=4;
    auto owned=guardian::hardcore::test_inventory_ids(nullptr,10,fake_inventory_setup);CHECK(owned.size()==1&&owned[0]==88);
    nested_setup.links[1]=10;nested_setup.categories[1]=5;bool cyclic=false;try{guardian::hardcore::test_inventory_ids(nullptr,10,fake_inventory_setup);}catch(const std::runtime_error&){cyclic=true;}CHECK(cyclic);
    return 0;
}
