// Guardian hardcore's integrated native backend. Engine contracts derived from EMF's MIT
// research; no EMF runtime/client dependency. See dependencies/native/THIRD_PARTY_NOTICES.txt.
#include "hardcore.h"
#include "hardcore_native.h"
#include "hardcore_loot.h"
#include "log.h"
#include <windows.h>
#include <safetyhook.hpp>
#include <algorithm>
#include <array>
#include <atomic>
#include <climits>
#include <cmath>
#include <cstring>
#include <map>
#include <mutex>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>
namespace guardian::hardcore {
namespace {
using namespace native;
thread_local const char* operation="initialization";
thread_local DWORD exception_code{};
thread_local uintptr_t exception_instruction{}, exception_address{};
bool read_checked(void* out,const void* source,size_t size){__try{std::memcpy(out,source,size);return true;}__except(EXCEPTION_EXECUTE_HANDLER){return false;}}
template<class T>T rd(const void* p,size_t off=0){T v;if(!read_checked(&v,static_cast<const uint8_t*>(p)+off,sizeof v))throw std::runtime_error("unreadable native memory");return v;}
template<class T>void wr(void* p,size_t off,T v){std::memcpy(static_cast<uint8_t*>(p)+off,&v,sizeof v);}
using GetComponent=void(*)(uint64_t*,uint8_t*,uint32_t,uint32_t);
using ReadOther=void*(*)(void**,void*,uint64_t,uint32_t);
using Register=uint64_t(*)(void*,const Program*,char);
using Begin=void(*)(void*,void*,uint32_t);
using Queue=uint8_t*(*)(void*,void*,uint8_t,uint32_t,uint32_t);
using FindAttr=void*(*)(void*,uint32_t,const void*);
using AddAttr=void(*)(void*,const void*,int32_t,uint32_t);
using Required=uint32_t(*)(uint32_t,const void*);
using Destroy=uint64_t(*)(void*,uint32_t,char,uint64_t);
struct Functions {GetComponent get{};ReadOther read{};Register reg{};Begin begin{};Queue queue{};FindAttr find{};AddAttr add{};Required required{};Destroy destroy{};
    void*(*ctor)(uint8_t*){};void(*init)(uint8_t*,const uint8_t*){};
    void*(*take)(void*,uint8_t*,uint32_t,uint32_t,uint64_t,uint32_t){};
    void*(*move)(void*,uint8_t*,uint64_t,uint64_t,int){};
    void*(*lift)(void*,uint8_t*,uint64_t,uint32_t,uint8_t){};
    void*(*put)(void*,uint8_t*,uint64_t,void*,uint32_t,uint32_t){};
    void(*commit)(uint8_t*){};void(*rollback)(uint8_t*){};void(*free)(uint8_t*){};void(*wait)(uint8_t*){};
} f;
extern "C" bool GuardianHardcoreEnabled();
extern "C" void GuardianHardcoreBackend(uint32_t);
extern "C" uint64_t GuardianHardcoreDeath(uint32_t,uint64_t);
extern "C" uint64_t GuardianHardcorePoll(uint32_t,uint64_t);
extern "C" bool GuardianHardcoreComplete(uint32_t,uint64_t,uint64_t);
struct Guardian {bool(*enabled)();void(*backend)(uint32_t);uint64_t(*death)(uint32_t,uint64_t);uint64_t(*poll)(uint32_t,uint64_t);bool(*complete)(uint32_t,uint64_t,uint64_t);};
Guardian guardian(){return {GuardianHardcoreEnabled,GuardianHardcoreBackend,GuardianHardcoreDeath,GuardianHardcorePoll,GuardianHardcoreComplete};}
SafetyHookInline sim_hook,register_hook,lift_hook,put_hook,commit_hook,rollback_hook,free_hook;
thread_local bool own_inventory_transaction=false;
struct OwnTransaction { bool before=own_inventory_transaction; OwnTransaction(){own_inventory_transaction=true;} ~OwnTransaction(){own_inventory_transaction=before;} };
std::array<SafetyHookMid,4> death_hooks;
std::atomic<uintptr_t> server_ptr{};
std::atomic<bool> faulted{},registered{};
struct Task {uint32_t handle{},entity{},start{};uint64_t account{},token{},began{};uint8_t before_count{};int phase{};Transform target{};bool wait_dead{};uint32_t stone{};uint64_t death_at{},loot_notice{};};
std::map<uint32_t,Task> tasks;
std::mutex tasks_mutex;
struct GraveLink {uint8_t* world{};uint32_t player{},handle{},stone{};uint64_t account{},seen{};};
std::map<uint64_t,GraveLink> grave_links;
Program descriptor{};
constexpr Name sreads[]={{"BalancingRegistry",17},{"g38_items::ItemRegistry",23},{"impact::AttributeRegistry",25}};
constexpr Name swrites[]={{"KnowledgeTriggerQueue",21}};
constexpr Name oreads[]={{"Actor",5},{"CurrentTransform",16},{"DestroyOnLoot",13},{"Health",6},{"LastTransform",13},{"PlayerSpawnPoint",16},{"Teleport",8},{"TeleportCount",13}};
constexpr Name owrites[]={{"Fuel",4},{"Inventory",9},{"InventoryEmptyState",19},{"InventorySetup",14},{"ItemState",9},{"OwnerRelationship",17},{"SlotSelection",13}};
constexpr Name addtypes[]={{"Teleport",8}};
constexpr Name events[]={{"InventoryFull",13},{"PickupEvent",11}};
constexpr size_t other_base=1+std::size(sreads)+std::size(swrites),write_base=other_base+std::size(oreads),event_base=write_base+std::size(owrites);
struct Component {void* ptr;uint64_t size;};
Component component(uint8_t* world,uint32_t entity,const char* name,size_t size){uint64_t out[2]{};std::string qualified="keen::ecs::";qualified+=name;f.get(out,world,hash(qualified.c_str()),entity);if(out[0]&&out[1]!=size)throw std::runtime_error(std::string("component layout changed: ")+name);return {reinterpret_cast<void*>(out[0]),out[1]};}
void* need(uint8_t* world,uint32_t entity,const char* name,size_t size){auto c=component(world,entity,name,size);if(!c.ptr)throw std::runtime_error(std::string("missing component: ")+name);return c.ptr;}
void* singleton(uint8_t* world,const char* name){auto entries=rd<uint8_t*>(world,0x7462d0);auto n=rd<uint64_t>(world,0x7462d8);if(!entries||n>4096)throw std::runtime_error("singleton table invalid");for(uint64_t i=0;i<n;++i){auto p=entries+i*0xe8;if(rd<uint32_t>(p,0xa0)==hash(name))return rd<void*>(p,0xa8);}return nullptr;}
bool session(uint8_t* world,Task& t){auto server=reinterpret_cast<uint8_t*>(server_ptr.load());if(!server||rd<uint8_t*>(server,0x1d0)!=world)return false;const auto index=t.handle&0x3f;if(index>=16)return false;auto slot=server+0x1e0+index*0x148;return rd<uint8_t>(slot,0xa)==3&&exact_session(t.handle,t.account,rd<uint32_t>(slot),rd<uint64_t>(slot,0x48))&&rd<uint32_t>(slot,0x18)==t.entity;}
int32_t* attribute(uint8_t* world,uint32_t entity,const char* type,size_t size,uint32_t id){auto comp=static_cast<uint8_t*>(need(world,entity,type,size));auto reg=singleton(world,"impact::AttributeRegistry");if(!reg)throw std::runtime_error("attribute registry missing");alignas(8) uint8_t result[16]{};f.find(result,id,reg);auto info=rd<uint8_t*>(result,8);if(result[0]||!info||rd<uint32_t>(info)!=rd<uint32_t>(comp)||(comp[6]&1)==0)throw std::runtime_error("attribute identity mismatch");auto slot=rd<uint16_t>(info,12),offset=rd<uint16_t>(comp,4);if(slot>=comp[7]||offset<8||offset+4ull*slot+4>size)throw std::runtime_error("attribute slot outside component");return reinterpret_cast<int32_t*>(comp+offset+4*slot);}
bool alive(uint8_t* world,uint32_t entity){auto actor=component(world,entity,"Actor",3832);if(!actor.ptr||(rd<uint64_t>(actor.ptr,0xbf8)&(uint64_t{1}<<50))==0)return false;return *attribute(world,entity,"Health",40,health_id)>0;}
void collect_inventories(uint8_t* world,uint32_t entity,std::vector<uint32_t>& ids,std::set<uint32_t>& path){if(path.size()>=16||!path.insert(entity).second)throw std::runtime_error("cyclic/too-deep inventory setup");auto setup=static_cast<Setup*>(need(world,entity,"InventorySetup",84));if(!setup->initialized)throw std::runtime_error("inventory not initialized");for(size_t i=0;i<16;++i){auto id=setup->links[i];if(!id)continue;if(setup->categories[i]==5)collect_inventories(world,id,ids,path);else if(std::find(ids.begin(),ids.end(),id)==ids.end()){if(ids.size()>=256)throw std::runtime_error("too many owned inventories");ids.push_back(id);}}path.erase(entity);}
std::vector<uint32_t> inventories(uint8_t* world,uint32_t entity){std::vector<uint32_t> ids;std::set<uint32_t> path;collect_inventories(world,entity,ids,path);return ids;}
bool empty(uint8_t* world,uint32_t entity){for(auto id:inventories(world,entity)){auto inv=static_cast<Inventory*>(need(world,id,"Inventory",96));for(const auto& item:inv->slots)if(item.item&&item.count)return false;}return true;}
bool skills_empty(uint8_t* world,uint32_t entity){auto nodes=static_cast<uint8_t*>(need(world,entity,"UnlockedSkillNodes",2316));for(int i=0;i<256;++i)if(nodes[2052+i]||rd<uint32_t>(nodes,1028+i*4))return false;return true;}
bool proof(uint8_t* world,uint32_t entity){return progress_proof(*attribute(world,entity,"Level",16,level_id),*attribute(world,entity,"Experience",20,xp_id),*attribute(world,entity,"Experience",20,pending_xp_id),empty(world,entity),skills_empty(world,entity));}
// StartLocation is resolved from the world's real spawn point, never hard-coded coordinates.
uint32_t start_point(uint8_t* world,Transform& transform){auto ecs=world+storage;auto registry=rd<uint8_t*>(ecs);if(!registry)return 0;auto types=rd<uint8_t*>(registry,8);auto count=rd<uint64_t>(registry,0x10);if(!types||count<0x100||count>0x280)throw std::runtime_error("type registry invalid");int type=-1;for(uint64_t i=0;i<count;++i)if(rd<uint32_t>(types+i*0x100,0x20)==hash("keen::ecs::PlayerSpawnPoint"))type=static_cast<int>(i);if(type<0)return 0;auto table=rd<uint8_t**>(ecs,0x130);auto capacity=rd<uint32_t>(ecs,0x140);if(!table||capacity>65536)return 0;std::vector<uint8_t*> sorted(table,table+capacity);std::sort(sorted.begin(),sorted.end());uint32_t found=0;
    for(uint32_t i=0;i<capacity;++i){auto arch=table[i];if(!arch||(reinterpret_cast<uintptr_t>(arch)&7)!=0||reinterpret_cast<uintptr_t>(arch)<0x10000||std::count(sorted.begin(),sorted.end(),arch)!=1)continue;if((rd<uint64_t>(arch,(type>>6)*8)&(uint64_t{1}<<(type&63)))==0)continue;auto offset=rd<uint16_t>(arch,0x84+type*2),size=rd<uint16_t>(arch,0xa84+type*2);if(size!=204)throw std::runtime_error("spawn-point size mismatch");auto chunk=rd<uint8_t*>(arch,0x50);for(int guard=0;chunk&&guard<20000;++guard,chunk=rd<uint8_t*>(chunk,8)){auto rows=rd<uint32_t>(chunk,0x10);auto ids=rd<uint16_t>(chunk,0x14);if(rows>8192||!ids)throw std::runtime_error("spawn chunk invalid");for(uint32_t j=0;j<rows;++j)if(rd<uint8_t>(chunk,offset+j*size)==0){auto id=rd<uint32_t>(chunk,ids+j*16);auto point=component(world,id,"CurrentTransform",56);if(!point.ptr)continue;if(found&&found!=id)return 0;found=id;std::memcpy(&transform,point.ptr,56);}}}return found;
}
bool is_tombstone(uint8_t* world,uint32_t entity){
    if(!entity)return false;auto map=world+storage+0x148;auto capacity=rd<uint64_t>(map,0x10);auto bits=rd<uint8_t*>(map,8);auto keys=rd<uint8_t*>(map,0x28);auto values=rd<uint8_t*>(map,0x40);
    if(!capacity||capacity>(1u<<22)||!bits||!keys||!values)return false;
    for(uint64_t i=0;i<capacity;++i){if(!(rd<uint64_t>(bits,(i>>6)*8)&(uint64_t{1}<<(i&63)))||rd<uint32_t>(keys,i*4)!=entity)continue;auto record=rd<uint8_t*>(values,i*8);if(!record)return false;auto chunk=rd<uint8_t*>(record,0x20);auto row=rd<uint32_t>(record,0x30);if(!chunk||row>=rd<uint32_t>(chunk,0x10))return false;auto infos=rd<uint16_t>(chunk,0x18);if(!infos)return false;auto info=rd<uint8_t*>(chunk,infos+row*8);if(!info)return false;auto text=rd<const char*>(info,0x10);auto size=rd<uint64_t>(info,0x18);constexpr char expected[]="Base_LootContainer_Player_LootCorpse_Tombstone";if(!text||size!=sizeof(expected)-1)return false;char name[sizeof expected]{};return read_checked(name,text,size)&&!std::memcmp(name,expected,size);}return false;
}

void teleport_components(uint8_t* world,uint32_t entity){need(world,entity,"CurrentTransform",sizeof(Transform));need(world,entity,"LastTransform",sizeof(LastTransform));need(world,entity,"TeleportCount",16);}
void validate_accessor(void* ctx,const uint64_t* iter,uint8_t* world,size_t index,uint32_t entity,const char* name,size_t size){auto job=rd<uint8_t*>(ctx);void* out[2]{};f.read(out,rd<void*>(job,0x30),iter[write_base+index],entity);auto actual=component(world,entity,name,size);if(!actual.ptr||out[0]!=actual.ptr)throw std::runtime_error(std::string("unverified inventory accessor: ")+name);}
bool clear_inventory(void* ctx,const uint64_t* iter,uint8_t* world,Task& task){const auto entity=task.entity;OwnTransaction own;operation="inventory accessor validation";auto ids=inventories(world,entity);validate_accessor(ctx,iter,world,3,entity,"InventorySetup",84);if(ids.empty())return true;validate_accessor(ctx,iter,world,1,ids[0],"Inventory",96);validate_accessor(ctx,iter,world,2,ids[0],"InventoryEmptyState",2);
    auto job=rd<void*>(ctx);if(iter[event_base]!=reinterpret_cast<uintptr_t>(job)||iter[event_base+1]!=reinterpret_cast<uintptr_t>(job))throw std::runtime_error("event writer/iterator mismatch");
    alignas(16) static uint8_t tx[0x71b0];alignas(16) uint8_t setup[0xf0]{};setup[0]=5;wr(setup,8,ctx);wr(setup,0x10,iter[2]);wr(setup,0x18,iter[1]);wr(setup,0x20,iter[3]);
    const std::pair<size_t,size_t> fields[]={{0x28,3},{0x30,1},{0x38,6},{0x40,4},{0x48,5},{0x50,3},{0x58,2},{0x70,0}};for(const auto& field:fields)wr(setup,field.first,iter[write_base+field.second]);wr(setup,0xa0,iter[event_base+1]);wr(setup,0xa8,iter[event_base]);wr(setup,0xc0,iter[4]);wr(setup,0xc8,entity);
    if(reinterpret_cast<void*>(iter[2])!=singleton(world,"g38_items::ItemRegistry")||reinterpret_cast<void*>(iter[3])!=singleton(world,"impact::AttributeRegistry"))throw std::runtime_error("inventory singleton slots mismatch");
    operation="equipped item transfer";
    std::vector<loot::Item> equipment;std::set<uint64_t> equipped_slots;
    auto selection=static_cast<uint8_t*>(need(world,entity,"SlotSelection",216));
    for(size_t i=0;i<20;++i){auto slot=rd<uint64_t>(selection,i*8);if(slot&&(slot>>32)<8&&std::find(ids.begin(),ids.end(),static_cast<uint32_t>(slot))!=ids.end())equipped_slots.insert(slot);}
    auto source_setup=static_cast<Setup*>(need(world,entity,"InventorySetup",84));
    for(size_t i=0;i<16;++i)if(source_setup->links[i]&&(source_setup->categories[i]==1||source_setup->categories[i]==2))for(uint32_t slot=0;slot<8;++slot)equipped_slots.insert(uint64_t{source_setup->links[i]}|(uint64_t{slot}<<32));
    for(auto slot:equipped_slots){auto inv=static_cast<Inventory*>(need(world,static_cast<uint32_t>(slot),"Inventory",96));auto stack=inv->slots[slot>>32];if(stack.item&&stack.count)equipment.push_back({slot,stack});}
    std::vector<loot::Move> moves;
    if(!equipment.empty()){
        if(!task.stone){if(GetTickCount64()-task.loot_notice>5000){log("HARDCORE %u: equipped items retained; waiting for an exact native death-to-tombstone transfer link.",task.handle);task.loot_notice=GetTickCount64();}return false;}
        auto destroy=component(world,task.stone,"DestroyOnLoot",104);
        if(!is_tombstone(world,task.stone)||!destroy.ptr||rd<uint8_t>(destroy.ptr,64)){task.stone=0;return false;}
        auto dest_setup=static_cast<Setup*>(need(world,task.stone,"InventorySetup",84));if(!dest_setup->initialized)return false;
        validate_accessor(ctx,iter,world,3,task.stone,"InventorySetup",84);
        std::vector<uint64_t> free_slots;uint32_t remaining=dest_setup->available?dest_setup->available:dest_setup->generic;
        for(auto id:inventories(world,task.stone)){auto inv=static_cast<Inventory*>(need(world,id,"Inventory",96));validate_accessor(ctx,iter,world,1,id,"Inventory",96);for(uint32_t slot=0;slot<8&&remaining;++slot,--remaining)if(!inv->slots[slot].item&&!inv->slots[slot].count&&!inv->slots[slot].persistent_id)free_slots.push_back(uint64_t{id}|(uint64_t{slot}<<32));}
        moves=loot::plan(equipment,free_slots);
        if(moves.size()!=equipment.size()){if(GetTickCount64()-task.loot_notice>5000){log("HARDCORE %u: tombstone %u has insufficient room; equipment and reset retained.",task.handle,task.stone);task.loot_notice=GetTickCount64();}return false;}
    }
    struct Take {uint32_t item,count;uint64_t slot;};std::vector<Take> entries;for(auto id:ids){auto inv=static_cast<Inventory*>(need(world,id,"Inventory",96));for(uint32_t slot=0;slot<8;++slot){auto item=inv->slots[slot];if(item.item&&item.count&&!equipped_slots.contains(uint64_t{id}|(uint64_t{slot}<<32)))entries.push_back({item.item,item.count,uint64_t{id}|(uint64_t{slot}<<32)});}}
    if(entries.empty()&&moves.empty())return true;operation="inventory transaction";setup[0]=0;f.ctor(tx);f.init(tx,setup);bool ok=true;
    for(const auto& move:moves){alignas(8) uint8_t result[16]{};f.move(result,tx,move.destination,move.source,0);if(result[0]||rd<uint32_t>(result,8)!=move.stack.count){ok=false;break;}}
    if(ok)for(const auto& entry:entries){alignas(8) uint8_t result[16]{};f.take(result,tx,entity,entry.item,entry.slot,entry.count);if(result[0]||rd<uint32_t>(result,8)){ok=false;break;}}if(ok)f.commit(tx);else f.rollback(tx);f.free(tx);if(!ok)throw std::runtime_error("atomic inventory removal refused");if(!empty(world,entity))throw std::runtime_error("inventory removal incomplete");
    for(const auto& move:moves){auto inv=static_cast<Inventory*>(need(world,static_cast<uint32_t>(move.destination),"Inventory",96));if(!loot::same(inv->slots[move.destination>>32],move.stack))throw std::runtime_error("tombstone transfer verification failed");}
    if(!moves.empty())log("HARDCORE %u: %zu equipped stacks moved to tombstone %u with persistent item identities preserved.",task.handle,moves.size(),task.stone);return true;
}
std::vector<uint32_t> skill_impacts(uint8_t* world,uint32_t entity){auto nodes=static_cast<uint8_t*>(need(world,entity,"UnlockedSkillNodes",2316));std::vector<uint32_t> impacts;for(int i=0;i<256;++i){auto id=rd<uint32_t>(nodes,1028+i*4);if(id&&std::find(impacts.begin(),impacts.end(),id)==impacts.end())impacts.push_back(id);}for(auto id:impacts){auto owner=component(world,id,"OwnerRelationship",4);if(id==entity||!owner.ptr||rd<uint32_t>(owner.ptr)!=entity)throw std::runtime_error("skill impact ownership unverified");}return impacts;}
void reset_progress(uint8_t* world,uint32_t entity){operation="native progression reset";auto balance=singleton(world,"BalancingRegistry");auto reg=singleton(world,"impact::AttributeRegistry");if(!balance||!reg)throw std::runtime_error("progress registries missing");auto required=f.required(1,balance);if(!required||required>INT32_MAX)throw std::runtime_error("level-one XP curve invalid");struct Change{const char* type;size_t size;uint32_t id;int32_t value;};const Change changes[]={{"Experience",20,required_xp_id,static_cast<int32_t>(required)},{"Experience",20,xp_id,0},{"Experience",20,pending_xp_id,0},{"Level",16,level_id,1}};
    int32_t previous[4]{};for(size_t i=0;i<4;++i)previous[i]=*attribute(world,entity,changes[i].type,changes[i].size,changes[i].id);
    auto impacts=skill_impacts(world,entity);auto nodes=static_cast<uint8_t*>(need(world,entity,"UnlockedSkillNodes",2316));
    for(size_t i=0;i<4;++i){const int64_t delta=int64_t{changes[i].value}-previous[i];if(delta<INT32_MIN||delta>INT32_MAX)throw std::runtime_error("progress delta overflow");if(delta)f.add(need(world,entity,changes[i].type,changes[i].size),reg,static_cast<int32_t>(delta),changes[i].id);}
    // After WaitWorldJobs: no jobs retain these component pointers. Destroy the
    // old skill-impact entities as well as ranks, so their stat buffs cannot survive.
    std::memset(nodes+1028,0,1024);std::memset(nodes+2052,0,256);wr(nodes,2308,rd<uint32_t>(nodes,2308)+1);nodes[2312]++;
    for(auto id:impacts)if(id!=entity)f.destroy(world+storage,id,1,0);
}
void teleport(void* ctx,uint8_t* world,Task& task){operation="native teleport";auto current=static_cast<Transform*>(need(world,task.entity,"CurrentTransform",56));auto counter=static_cast<uint8_t*>(need(world,task.entity,"TeleportCount",16));auto job=rd<uint8_t*>(ctx);auto buffer=rd<uint8_t*>(job,8);if(!buffer||rd<uint64_t>(buffer,0x18)>uint64_t{1}<<30||rd<uint32_t>(buffer,0x20)>rd<uint64_t>(buffer,0x18))throw std::runtime_error("command buffer invalid");
    Teleport data{};data.target=*current;data.target.position[0]=task.target.position[0];data.target.position[1]=task.target.position[1];data.target.position[2]=task.target.position[2];data.entity=task.start;data.ground=1;
    auto rec=f.queue(buffer,buffer+0x10,3,0x6c+sizeof data,task.entity);if(!rec)return;wr(rec,0x0c,hash("keen::ecs::Teleport"));wr<uint16_t>(rec,0x4c,sizeof data);std::memcpy(rec+0x6c,&data,sizeof data);task.before_count=*counter;task.phase=3;
    // A native checkpoint asks clients to accept the authoritative server state.
    auto checkpoint=f.queue(buffer,buffer+0x10,0x10,0x10,task.entity);if(checkpoint)wr(checkpoint,0x0c,task.entity);
}
void fail(const char* why){faulted.store(true);log("HARDCORE disabled at %s: %s. Pending resets/backups retained; no kick requested.",operation,why);}
void program_run(void* ctx){if(faulted.load())return;auto g=guardian();if(!g.enabled||!g.enabled())return;auto job=rd<uint8_t*>(ctx);auto data=rd<uint8_t*>(job);if(!data||data[8]!=0)return;auto world=data-storage;if(world[0]!=2)return;g.backend(1);auto server=reinterpret_cast<uint8_t*>(server_ptr.load());if(!server||rd<uint8_t*>(server,0x1d0)!=world)return;
    operation="ECS iterator";uint64_t iter[128]{};f.begin(ctx,iter,sizeof iter);std::lock_guard lock(tasks_mutex);
    for(int i=0;i<16;++i){auto slot=server+0x1e0+i*0x148;const auto handle=rd<uint32_t>(slot);const auto account=rd<uint64_t>(slot,0x48);const auto entity=rd<uint32_t>(slot,0x18);if(!handle||!account||!entity||rd<uint8_t>(slot,0xa)!=3)continue;operation="respawn state";auto prior=tasks.find(handle);if(prior!=tasks.end()&&prior->second.account==account&&prior->second.entity==entity&&prior->second.wait_dead&&!alive(world,entity))prior->second.wait_dead=false;auto token=g.poll(handle,account);if(!token)continue;
        auto& task=tasks[handle];if(task.token!=token||task.account!=account||task.entity!=entity)task=Task{handle,entity,0,account,token,0};if(task.phase==4)continue;if(task.phase>0&&GetTickCount64()-task.began>60000){log("HARDCORE %u: timed out waiting for verified native respawn/teleport; pending reset retained.",handle);task.phase=4;continue;}
        if(task.wait_dead||!alive(world,entity)||component(world,entity,"Teleport",80).ptr)continue;
        if(task.phase==0){operation="initial spawn lookup";task.start=start_point(world,task.target);if(!task.start)continue;
            // Preflight all progression structures before touching inventory.
            operation="progression preflight";attribute(world,entity,"Level",16,level_id);attribute(world,entity,"Experience",20,xp_id);attribute(world,entity,"Experience",20,pending_xp_id);attribute(world,entity,"Experience",20,required_xp_id);need(world,entity,"UnlockedSkillNodes",2316);teleport_components(world,entity);
            operation="skill effect ownership";skill_impacts(world,entity);if(clear_inventory(ctx,iter,world,task)){task.began=GetTickCount64();task.phase=1;}
        }else if(task.phase==2&&proof(world,entity)){teleport(ctx,world,task);}
    }
}
void post_run(uint8_t* data){if(faulted.load()||!data||data[8]!=0)return;auto g=guardian();if(!g.enabled||!g.enabled())return;auto world=data-storage;if(world[0]!=2)return;f.wait(world);std::lock_guard lock(tasks_mutex);for(auto it=tasks.begin();it!=tasks.end();){auto& task=it->second;if(!session(world,task)){it=tasks.erase(it);continue;}if(task.phase==1){reset_progress(world,task.entity);if(!proof(world,task.entity))throw std::runtime_error("native reset verification failed");task.phase=2;}else if(task.phase==3){auto counter=static_cast<uint8_t*>(need(world,task.entity,"TeleportCount",16));auto current=static_cast<Transform*>(need(world,task.entity,"CurrentTransform",56));if(*counter!=task.before_count&&!component(world,task.entity,"Teleport",80).ptr&&alive(world,task.entity)&&near_start(*current,task.target)&&proof(world,task.entity)){if(g.complete(task.handle,task.account,task.token))log("HARDCORE %u: level/XP, empty inventory, skills and native start-point teleport verified without disconnect.",task.handle);task.phase=4;}}++it;}}
int native_exception_filter(EXCEPTION_POINTERS* info){
    // MSVC exceptions must reach the C++ catch and unwind lock guards normally.
    if(info->ExceptionRecord->ExceptionCode==0xe06d7363)return EXCEPTION_CONTINUE_SEARCH;
    exception_code=info->ExceptionRecord->ExceptionCode;
    exception_instruction=reinterpret_cast<uintptr_t>(info->ExceptionRecord->ExceptionAddress);
    exception_address=info->ExceptionRecord->NumberParameters>1?info->ExceptionRecord->ExceptionInformation[1]:0;
    return EXCEPTION_EXECUTE_HANDLER;
}
bool seh(void(*fn)(void*),void* arg){exception_code=0;__try{fn(arg);return true;}__except(native_exception_filter(GetExceptionInformation())){return false;}}
void native_fault(const char* where){log("HARDCORE native exception: code=%08lx instruction=%llx address=%llx stage=%s",exception_code,static_cast<unsigned long long>(exception_instruction),static_cast<unsigned long long>(exception_address),operation);fail(where);}
void program(void* ctx){try{if(!seh(program_run,ctx))native_fault("native program access fault");}catch(const std::exception& e){fail(e.what());}catch(...){fail("native program exception");}}
void post(void* data){post_run(static_cast<uint8_t*>(data));}
void step(void* data,char ticked){sim_hook.call<void>(data,ticked);try{if(!seh(post,data))native_fault("native post-step access fault");}catch(const std::exception&e){fail(e.what());}catch(...){fail("native post-step exception");}}

struct PutWitness {loot::Take source;uint32_t stone;uint64_t slot;Stack expected;};
struct Trace {uint8_t* world{};std::vector<loot::Take> takes;std::vector<PutWitness> puts;};
std::map<uint8_t*,Trace> traces;
std::mutex traces_mutex;
uint8_t* transaction_world(uint8_t* tx){auto server=reinterpret_cast<uint8_t*>(server_ptr.load());if(!server)return nullptr;auto world=rd<uint8_t*>(server,0x1d0);if(!world||world[0]!=2)return nullptr;auto ctx=rd<uint8_t*>(tx);if(!ctx)return nullptr;auto job=rd<uint8_t*>(ctx);if(!job||rd<uint8_t*>(job)!=world+storage)return nullptr;return world;}
uint32_t root_owner(uint8_t* world,uint32_t id){std::set<uint32_t> seen;for(int i=0;id&&i<16;++i){if(!seen.insert(id).second)return 0;auto owner=component(world,id,"OwnerRelationship",4);if(!owner.ptr)return id;auto next=rd<uint32_t>(owner.ptr);if(!next||next==id)return id;id=next;}return 0;}
loot::Take source_player(uint8_t* world,uint32_t inventory){loot::Take result{};auto root=root_owner(world,inventory);auto server=reinterpret_cast<uint8_t*>(server_ptr.load());if(!root||!server)return result;for(int i=0;i<16;++i){auto slot=server+0x1e0+i*0x148;if(rd<uint8_t>(slot,0xa)==3&&rd<uint32_t>(slot,0x18)==root){result.player=root;result.handle=rd<uint32_t>(slot);result.account=rd<uint64_t>(slot,0x48);return result;}}return result;}
struct LiftArgs {void* out;uint8_t* tx;uint64_t slot;};
void observe_lift(void* arg){auto& a=*static_cast<LiftArgs*>(arg);if(rd<uint8_t>(a.out)||!rd<uint32_t>(a.out,16))return;auto world=transaction_world(a.tx);if(!world)return;auto who=source_player(world,static_cast<uint32_t>(a.slot));if(!who.player||!who.account)return;
    bool dead=*attribute(world,who.player,"Health",40,health_id)<=0;if(!dead){std::lock_guard lock(tasks_mutex);auto it=tasks.find(who.handle);dead=it!=tasks.end()&&it->second.account==who.account&&it->second.death_at&&GetTickCount64()-it->second.death_at<2000;}if(!dead)return;
    who.item=rd<uint32_t>(a.out,8);who.pide=rd<uint32_t>(a.out,12);who.count=rd<uint32_t>(a.out,16);std::lock_guard lock(traces_mutex);if(traces.size()>=128&&!traces.contains(a.tx))return;auto& trace=traces[a.tx];trace.world=world;trace.takes.push_back(who);
}
struct PutArgs {void* out;uint8_t* tx;uint64_t slot;void* info;uint32_t pide,count;};
void observe_put(void* arg){auto& a=*static_cast<PutArgs*>(arg);if(rd<uint8_t>(a.out)||!a.info||!a.count)return;auto missing=rd<uint32_t>(a.out,8);if(missing>=a.count)return;{std::lock_guard lock(traces_mutex);auto it=traces.find(a.tx);if(it==traces.end()||it->second.takes.empty())return;}auto world=transaction_world(a.tx);if(!world)return;auto stone=root_owner(world,static_cast<uint32_t>(a.slot));if(!is_tombstone(world,stone))return;
    std::lock_guard lock(traces_mutex);auto it=traces.find(a.tx);if(it==traces.end()||it->second.world!=world)return;auto item=rd<uint32_t>(a.info);auto moved=a.count-missing;auto source=loot::source(it->second.takes,item,a.pide,moved);if(!source)return;it->second.puts.push_back({*source,stone,a.slot,{item,moved,a.pide}});const_cast<loot::Take*>(source)->count-=moved;
}
void observe_commit(void* raw){auto tx=static_cast<uint8_t*>(raw);Trace trace;{std::lock_guard lock(traces_mutex);auto it=traces.find(tx);if(it==traces.end())return;trace=std::move(it->second);traces.erase(it);}for(const auto& put:trace.puts){auto world=transaction_world(tx);if(world!=trace.world)continue;auto live=source_player(world,put.source.player);if(live.handle!=put.source.handle||live.account!=put.source.account)continue;auto inv=component(world,static_cast<uint32_t>(put.slot),"Inventory",96);if(!inv.ptr||(put.slot>>32)>=8)continue;auto stack=static_cast<Inventory*>(inv.ptr)->slots[put.slot>>32];if(stack.item!=put.expected.item||stack.persistent_id!=put.expected.persistent_id||stack.count<put.expected.count)continue;
    std::lock_guard lock(tasks_mutex);GraveLink link{world,put.source.player,put.source.handle,put.stone,put.source.account,GetTickCount64()};auto previous=grave_links.find(link.account);if(previous!=grave_links.end()&&previous->second.stone!=link.stone&&link.seen-previous->second.seen<1000){previous->second=link;previous->second.stone=0;continue;}const bool new_link=previous==grave_links.end()||previous->second.stone!=link.stone||link.seen-previous->second.seen>2000;grave_links[link.account]=link;if(new_link)log("HARDCORE %u: native committed death inventory transfer verified tombstone %u.",link.handle,link.stone);auto task=tasks.find(link.handle);if(task!=tasks.end()&&task->second.account==link.account&&task->second.entity==link.player&&task->second.phase==0&&task->second.death_at&&link.seen+1000>=task->second.death_at&&link.seen<=task->second.death_at+10000)task->second.stone=link.stone;
}}
void discard_trace(uint8_t* tx){std::lock_guard lock(traces_mutex);traces.erase(tx);}
void observe(void(*fn)(void*),void* arg){try{if(!seh(fn,arg))log("HARDCORE: native tombstone witness skipped after access fault.");}catch(const std::exception&e){log("HARDCORE: tombstone witness skipped: %s",e.what());}}
bool observing(){auto g=guardian();return !own_inventory_transaction&&!faulted.load()&&g.enabled&&g.enabled();}
void* lift_items(void* out,uint8_t* tx,uint64_t slot,uint32_t count,uint8_t clear){auto result=lift_hook.call<void*>(out,tx,slot,count,clear);if(observing()){LiftArgs args{out,tx,slot};observe(observe_lift,&args);}return result;}
void* put_items(void* out,uint8_t* tx,uint64_t slot,void* info,uint32_t pide,uint32_t count){auto result=put_hook.call<void*>(out,tx,slot,info,pide,count);if(observing()){PutArgs args{out,tx,slot,info,pide,count};observe(observe_put,&args);}return result;}
void commit_items(uint8_t* tx){commit_hook.call<void>(tx);if(observing())observe(observe_commit,tx);else discard_trace(tx);}
void rollback_items(uint8_t* tx){rollback_hook.call<void>(tx);discard_trace(tx);}
void free_items(uint8_t* tx){free_hook.call<void>(tx);discard_trace(tx);}

void death_event_run(void* raw){if(faulted.load()||!registered.load())return;auto& ctx=*static_cast<SafetyHookContext*>(raw);const auto account=rd<uint64_t>(reinterpret_cast<void*>(ctx.r8));if(!account)return;auto server=reinterpret_cast<uint8_t*>(server_ptr.load());if(!server)return;for(int i=0;i<16;++i){auto slot=server+0x1e0+i*0x148;if(rd<uint8_t>(slot,0xa)==3&&rd<uint64_t>(slot,0x48)==account&&rd<uint32_t>(slot,0x18)){death(rd<uint32_t>(slot),account);return;}}}
void death_event(SafetyHookContext& ctx){try{if(!seh(death_event_run,&ctx))native_fault("death telemetry access fault");}catch(const std::exception& e){fail(e.what());}}
void all_programs(void* world){register_hook.call<void>(world);if(faulted.load())return;try{const auto result=f.reg(world,&descriptor,0);if(result&0xff){registered.store(true);log("HARDCORE native ECS program registered; inactive unless guardian.json hardcore=true.");}else fail("engine refused hardcore program registration");}catch(...){fail("hardcore program registration exception");}}
uintptr_t find(const uint8_t* image,const char* pattern,uint32_t expected){std::vector<int> bytes;std::string p(pattern);for(size_t at=0;at<p.size();){if(p[at]==' '){++at;continue;}if(p[at]=='?'){bytes.push_back(-1);while(at<p.size()&&p[at]=='?')++at;}else{bytes.push_back(std::stoi(p.substr(at,2),nullptr,16));at+=2;}}
    const auto pe=rd<uint32_t>(image,0x3c);const auto section=image+pe+24+rd<uint16_t>(image,pe+20);const auto count=rd<uint16_t>(image,pe+6);uintptr_t match=0;for(uint16_t s=0;s<count;++s){auto head=section+s*40;if(std::memcmp(head,".text",5))continue;auto rva=rd<uint32_t>(head,12),len=rd<uint32_t>(head,8);for(uint32_t i=0;i+bytes.size()<=len;++i){bool same=true;for(size_t k=0;k<bytes.size();++k)if(bytes[k]>=0&&image[rva+i+k]!=bytes[k]){same=false;break;}if(same){if(match)throw std::runtime_error("native pattern is ambiguous");match=reinterpret_cast<uintptr_t>(image)+rva+i;}}}if(!match||match!=reinterpret_cast<uintptr_t>(image)+expected)throw std::runtime_error("native pattern/RVA mismatch at expected " + std::to_string(expected));return match;
}
}
#ifdef HARDCORE_TEST
void throw_test_validation(void*){throw std::runtime_error("specific validation reason");}
void raise_test_fault(void*){ULONG_PTR data[2]={0,0x1234};RaiseException(EXCEPTION_ACCESS_VIOLATION,0,2,data);}
bool test_native_diagnostics(){bool validation=false;try{seh(throw_test_validation,nullptr);}catch(const std::runtime_error& e){validation=std::string(e.what())=="specific validation reason";}return validation&&!seh(raise_test_fault,nullptr)&&exception_code==EXCEPTION_ACCESS_VIOLATION&&exception_address==0x1234;}
std::vector<uint32_t> test_inventory_ids(uint8_t* world,uint32_t entity,void(*get)(uint64_t*,uint8_t*,uint32_t,uint32_t)){f.get=get;return inventories(world,entity);}
bool test_is_tombstone(uint8_t* world,uint32_t entity){return is_tombstone(world,entity);}
void test_teleport_components(uint8_t* world,void(*get)(uint64_t*,uint8_t*,uint32_t,uint32_t)){f.get=get;teleport_components(world,91);}
uint32_t test_start(uint8_t* world,void(*get)(uint64_t*,uint8_t*,uint32_t,uint32_t)){f.get=get;Transform t{};return start_point(world,t);}
#endif
void manager(uintptr_t value){server_ptr.store(value);}
void death(uint32_t handle,uint64_t account){if(faulted.load()||!registered.load())return;auto g=guardian();if(g.enabled&&g.enabled()){g.backend(1);const auto token=g.death(handle,account);if(!token){log("HARDCORE death ignored for handle %u: no verified Guardian identity/reference.",handle);return;}auto server=reinterpret_cast<uint8_t*>(server_ptr.load());if(!server||(handle&0x3f)>=16)return;auto slot=server+0x1e0+(handle&0x3f)*0x148;if(!exact_session(handle,account,rd<uint32_t>(slot),rd<uint64_t>(slot,0x48)))return;auto entity=rd<uint32_t>(slot,0x18);if(!entity)return;std::lock_guard lock(tasks_mutex);auto& task=tasks[handle];if(task.token!=token){task=Task{handle,entity,0,account,token,0};task.wait_dead=true;}task.death_at=GetTickCount64();auto link=grave_links.find(account);if(link!=grave_links.end()&&link->second.world==rd<uint8_t*>(server,0x1d0)&&link->second.player==entity&&link->second.handle==handle&&task.death_at-link->second.seen<1000)task.stone=link->second.stone;}}
bool resolve(const uint8_t* image,bool probe_only){try{const auto pe=rd<uint32_t>(image,0x3c);if(rd<uint32_t>(image,pe+8)!=0x6aeb143e||rd<uint32_t>(image,pe+24+56)!=0x1dde000){log("HARDCORE unavailable on unverified executable; core Guardian hooks unaffected.");return false;}
#define RESOLVE(field,rva,pattern) f.field=reinterpret_cast<decltype(f.field)>(find(image,pattern,rva))
    RESOLVE(get,0x64ed50,"48 89 5C 24 08 48 89 74 24 10 57 48 83 EC 20 48 8D BA ?? ?? ?? 00 44 89");
    RESOLVE(read,0x64e9e0,"40 53 48 83 EC 20 48 8B D9 E8 ?? ?? ?? ?? 48 8B C3 48 83 C4 20 5B C3 CC CC CC CC CC CC CC CC CC 4C 8B D1 4D");
    RESOLVE(reg,0x67ede0,"48 89 5C 24 08 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8D 6C 24 C9 48 81 EC A0 00");
    RESOLVE(begin,0x663b30,"48 89 5C 24 10 55 56 57 41 54 41 55 41 56 41 57 48 83 EC 60 4C 8B 09 48");
    RESOLVE(queue,0x5f9f60,"48 89 74 24 10 57 48 83 EC 20 44 8B 52 10 48 8B FA 41 8B C2 83 E0 0F 74 07 44 2B D0 41 83 C2 10");
    RESOLVE(find,0x991810,"89 54 24 10 53 48 83 EC 20 48 8B D9 48 8D 54 24 38 49 8D 48 68");
    RESOLVE(required,0x93e900,"4C 8B 4A 20 44 8B D1 4D 85 C9 75 06 B8 01 00 00 00 C3 41 83 FA 05 77 0D 41 8D 4A FF 41 8B 84 89 D8 01 00 00 C3");
    RESOLVE(destroy,0x63e8a0,"48 89 5C 24 18 48 89 74 24 20 89 54 24 10 55 57 41 55 41 56 41 57 48 8D");
    RESOLVE(ctor,0x3ed10,"33 D2 33 C0 48 89 11 48 89 51 08 48 89 51 10 48 89 51 18 48 89 51 20 48 89 51 28 48 89 51 30 48 89 51 38 48 89 51 40 48 89 51 48 48 89 51 50 48 89 51 58 48 89 51 60 48 89 51 68 48 89 51 70 48 89 51 78 48");
    RESOLVE(init,0x1b9300,"C6 81 A8 71 00 00 00 48 8B 42 08 48");RESOLVE(take,0x1b0d90,"48 89 5C 24 08 48 89 74 24 18 48 89 7C 24 20 48 89 54 24 10 55 41 54 41 55 41 56 41 57 48 8D AC 24 20 FF FF");
    RESOLVE(move,0x1b4550,"48 89 5C 24 18 55 56 57 41 56 41 57 48 8B EC 48");
    RESOLVE(lift,0x1b9460,"40 53 55 57 41 54 41 56 48 83 EC 60");
    RESOLVE(put,0x1b7100,"48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 57 41 56 41 57 48 83 EC 30 4D 8B F1 49");
    RESOLVE(commit,0x1af860,"40 55 53 41 55 48 8D AC 24 30 FC FF");RESOLVE(rollback,0x1b8310,"4C 8B DC 55 56 41 55 41 56 41 57 49");RESOLVE(free,0x3ee60,"48 89 5C 24 08 48 89 74 24 10 57 48 83 EC 20 48 8B F9 45 33 C0 48 81 C1 40 71 00 00");RESOLVE(wait,0x6ca080,"48 8B 89 ?? ?? E1 00 E9 ?? ?? ?? ??");
#undef RESOLVE
    const auto attr_call=find(image,"44 8B 0D ?? ?? ?? ?? 44 8B C6 48 8B 55 A0 49 8B CE E8 ?? ?? ?? ??",0x191521)+17;f.add=reinterpret_cast<AddAttr>(attr_call+5+rd<int32_t>(reinterpret_cast<void*>(attr_call),1));if(reinterpret_cast<uintptr_t>(f.add)!=reinterpret_cast<uintptr_t>(image)+0x190810)throw std::runtime_error("attribute writer target mismatch");
    const auto sim=find(image,"40 55 53 56 41 55 48 8D AC 24 F8 FF FD FF B8 08 01 02 00 E8 ?? ?? ?? ??",0x6c8f10);
    const auto all=find(image,"40 53 48 83 EC 50 48 8B D9 48 8D 15 ?? ?? ?? ?? 48 8D 4C 24 40 41 B8 24",0x245960);
    descriptor.name={"guardian_hardcore",17};descriptor.update=program;descriptor.sys_read={sreads,std::size(sreads)};descriptor.sys_write={swrites,std::size(swrites)};descriptor.other_read={oreads,std::size(oreads)};descriptor.other_write={owrites,std::size(owrites)};descriptor.add={addtypes,1};descriptor.events_out={events,2};descriptor.flags=0x14;descriptor.interval=1;
    constexpr uint32_t death_calls[]={0x59d6f,0x8a87d,0xc2714,0x191b29};
    constexpr uint8_t death_type[]={0xb9,0x53,0x56,0x6d,0x66,0xe8};
    if(std::memcmp(image+0x52430+0x1a3,death_type,sizeof death_type))throw std::runtime_error("death event type marker changed");
    for(auto rva:death_calls){if(image[rva]!=0xe8||static_cast<int64_t>(rva)+5+rd<int32_t>(image,rva+1)!=0x52430)throw std::runtime_error("death callsite target changed");}
    if(probe_only)return true;
    sim_hook=safetyhook::create_inline(reinterpret_cast<void*>(sim),reinterpret_cast<void*>(step));if(!sim_hook)throw std::runtime_error("sim-step hook failed");register_hook=safetyhook::create_inline(reinterpret_cast<void*>(all),reinterpret_cast<void*>(all_programs));if(!register_hook){sim_hook.reset();throw std::runtime_error("program-registration hook failed");}
    for(size_t i=0;i<death_hooks.size();++i){death_hooks[i]=safetyhook::create_mid(const_cast<uint8_t*>(image)+death_calls[i],death_event);if(!death_hooks[i]){for(auto& hook:death_hooks)hook.reset();register_hook.reset();sim_hook.reset();throw std::runtime_error("death event callsite hook failed");}}
    lift_hook=safetyhook::create_inline(reinterpret_cast<void*>(f.lift),reinterpret_cast<void*>(lift_items));put_hook=safetyhook::create_inline(reinterpret_cast<void*>(f.put),reinterpret_cast<void*>(put_items));commit_hook=safetyhook::create_inline(reinterpret_cast<void*>(f.commit),reinterpret_cast<void*>(commit_items));rollback_hook=safetyhook::create_inline(reinterpret_cast<void*>(f.rollback),reinterpret_cast<void*>(rollback_items));free_hook=safetyhook::create_inline(reinterpret_cast<void*>(f.free),reinterpret_cast<void*>(free_items));
    if(!lift_hook||!put_hook||!commit_hook||!rollback_hook||!free_hook)throw std::runtime_error("tombstone inventory witness hooks failed");
    log("HARDCORE backend hooks installed for verified server build; no client mod or EMF runtime required.");return true;
}catch(const std::exception&e){lift_hook.reset();put_hook.reset();commit_hook.reset();rollback_hook.reset();free_hook.reset();for(auto& hook:death_hooks)hook.reset();sim_hook.reset();register_hook.reset();log("HARDCORE backend unavailable: %s. Core Guardian hooks unaffected.",e.what());return false;}}
bool install(const uint8_t* image){return resolve(image,false);}
bool probe(const uint8_t* image){return resolve(image,true);}
}

extern "C" bool guardian_native_initialize(const uint8_t* image){return guardian::hardcore::install(image);}
extern "C" void guardian_native_manager(uintptr_t manager){guardian::hardcore::manager(manager);}
extern "C" void guardian_native_death(uint32_t handle,uint64_t account){guardian::hardcore::death(handle,account);}
