#include "hardcore_loot.h"
#include <cstdio>
using namespace guardian::hardcore::loot;
int main(){
#define CHECK(x) if(!(x)){std::fprintf(stderr,"loot test failed %d\n",__LINE__);return 1;}
    Item ring{uint64_t{10}|(uint64_t{1}<<32),{100,1,999}};Item boots{uint64_t{10}|(uint64_t{2}<<32),{101,1,1000}};
    auto moves=plan({ring,boots},{20,20|(uint64_t{1}<<32)});CHECK(moves.size()==2);CHECK(same(moves[0].stack,ring.stack));CHECK(moves[0].stack.persistent_id==999);CHECK(plan({ring,boots},{20}).empty());
    bool duplicate=false;try{plan({ring,ring},{20,21});}catch(const std::runtime_error&){duplicate=true;}CHECK(duplicate);
    std::vector<Take> takes{{1,64,100,999,1,123}};CHECK(source(takes,100,999,1));CHECK(!source(takes,100,998,1));CHECK(!source(takes,100,999,2));takes.push_back({2,65,100,999,1,456});CHECK(!source(takes,100,999,1));
    return 0;
}
