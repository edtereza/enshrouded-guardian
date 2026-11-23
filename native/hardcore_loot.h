#pragma once
#include "hardcore_native.h"
#include <vector>
#include <set>
#include <stdexcept>
namespace guardian::hardcore::loot {
using native::Stack;
struct Item {uint64_t slot;Stack stack;};
struct Move {uint64_t source,destination;Stack stack;};
inline bool same(const Stack&a,const Stack&b){return a.item==b.item&&a.count==b.count&&a.persistent_id==b.persistent_id;}
inline std::vector<Move> plan(const std::vector<Item>& gear,const std::vector<uint64_t>& free_slots){
    if(free_slots.size()<gear.size())return {};
    std::set<uint64_t> all_sources;for(const auto& item:gear)all_sources.insert(item.slot);
    std::set<uint64_t> sources,destinations;std::set<uint32_t> identities;std::vector<Move> out;
    for(size_t i=0;i<gear.size();++i){auto item=gear[i];const auto dst=free_slots[i];
        if(!item.slot||!dst||(item.slot>>32)>=8||(dst>>32)>=8||all_sources.contains(dst)||!item.stack.item||!item.stack.count||!sources.insert(item.slot).second||!destinations.insert(dst).second)throw std::runtime_error("invalid tombstone transfer plan");
        if(item.stack.persistent_id&&!identities.insert(item.stack.persistent_id).second)throw std::runtime_error("duplicate equipment identity");
        out.push_back({item.slot,dst,item.stack});
    }return out;
}
struct Take {uint32_t player,handle,item,pide,count;uint64_t account;};
inline const Take* source(const std::vector<Take>& takes,uint32_t item,uint32_t pide,uint32_t count){
    const Take* found=nullptr;for(const auto& take:takes)if(take.item==item&&take.pide==pide&&take.count>=count){if(found&&(found->player!=take.player||found->account!=take.account||found->handle!=take.handle))return nullptr;found=&take;}return found;
}
}
