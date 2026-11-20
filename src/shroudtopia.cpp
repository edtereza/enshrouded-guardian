#include <string>
#include <cstdint>
#include <cstring>
#include <windows.h>

// Read-only character-name lookup, based on EMF player_strings / WorldView.
// Guard every engine read and verify the text's FNV id before returning it.
static uint32_t name_hash(const char* text) {
    uint32_t value = 2166136261u;
    while (*text) { value = (value ^ static_cast<uint8_t>(*text++)) * 16777619u; }
    return value;
}

static uint8_t* named_singleton(uint8_t* world, const char* name) {
    auto entries = *reinterpret_cast<uint8_t**>(world + 0x7462D0);
    auto count = *reinterpret_cast<uint64_t*>(world + 0x7462D8);
    if (!entries || count > 4096) return nullptr;
    const auto hash = name_hash(name);
    for (uint64_t i = 0; i < count; ++i) {
        auto entry = entries + i * 0xE8;
        if (*reinterpret_cast<uint32_t*>(entry + 0xA0) == hash)
            return *reinterpret_cast<uint8_t**>(entry + 0xA8);
    }
    return nullptr;
}

extern "C" bool guardian_character_name(uint8_t* server, uint32_t handle, uint64_t account,
    uintptr_t component_fn, char* output, size_t capacity) {
    if (!server || !component_fn || !output || capacity < 257) return false;
    __try {
        auto world = *reinterpret_cast<uint8_t**>(server + 0x1D0);
        if (!world || world[0] != 2 || world[0xE179C8] != 0) return false;
        auto infos = named_singleton(world, "BasicPlayerInfos");
        auto strings = named_singleton(world, "CustomPlayerStringsSystem");
        if (!infos || !strings) return false;
        uint32_t player = 0;
        for (uint32_t i = 0; i < 1023; ++i) {
            auto entry = infos + i * 0xA0;
            if (*reinterpret_cast<uint32_t*>(entry) == i + 1 &&
                *reinterpret_cast<uint32_t*>(entry + 4) == handle &&
                *reinterpret_cast<uint64_t*>(entry + 8) == account) {
                if (player) return false;
                player = i + 1;
            }
        }
        if (!player) return false;
        using GetComponent = void(__fastcall*)(uint64_t*, uint8_t*, uint32_t, uint32_t);
        uint64_t component[2] = {};
        reinterpret_cast<GetComponent>(component_fn)(component, world, 0x0e699564, player);
        if (!component[0] || component[1] < 8) return false;
        const auto id = *reinterpret_cast<uint64_t*>(component[0]);
        auto map = strings + 0x190;
        auto bits = *reinterpret_cast<uint64_t**>(map + 8);
        auto slots = *reinterpret_cast<uint64_t*>(map + 0x10);
        auto keys = *reinterpret_cast<uint64_t**>(map + 0x28);
        auto values = *reinterpret_cast<uint8_t***>(map + 0x40);
        if (!id || !slots || slots > (1u << 20) || (slots & (slots - 1)) || !bits || !keys || !values) return false;
        uint64_t slot = static_cast<uint32_t>(id) & (slots - 1);
        for (uint64_t probes = 0; probes < slots && ((bits[slot >> 6] >> (slot & 63)) & 1); ++probes) {
            if (keys[slot] == id) {
                auto entry = values[slot];
                if (!entry || *reinterpret_cast<uint64_t*>(entry + 0x10) != id) return false;
                auto text = *reinterpret_cast<const char**>(entry + 0x18);
                auto length = *reinterpret_cast<uint64_t*>(entry + 0x20);
                if (!text || !length || length > 256) return false;
                uint64_t hash = 14695981039346656037ull;
                for (uint64_t i = 0; i < length; ++i) hash = (hash ^ static_cast<uint8_t>(text[i])) * 1099511628211ull;
                if (hash != id) return false;
                std::memcpy(output, text, static_cast<size_t>(length));
                output[length] = 0;
                return true;
            }
            slot = (slot + 1) & (slots - 1);
        }
    } __except (EXCEPTION_EXECUTE_HANDLER) { return false; }
    return false;
}

// Shroudtopia's C++ ABI, as published in include/shroudtopia.h.
// Keep this boundary in C++ so the loader receives native MSVC strings and
// virtual calls. The ModContext is opaque to the Guardian.
struct ModContext;

struct ModMetaData {
    std::string name;
    std::string description;
    std::string version;
    std::string author;
    std::string targetShroudtopiaVersion;
    bool hasClientSupport;
    bool hasServerSupport;
};

class Mod {
public:
    bool loaded = false;
    bool active = false;

    virtual ~Mod() = default;
    virtual ModMetaData GetMetaData() = 0;
    virtual void Load(ModContext*) = 0;
    virtual void Unload(ModContext*) = 0;
    virtual void Activate(ModContext*) = 0;
    virtual void Deactivate(ModContext*) = 0;
    virtual void Update(ModContext*) = 0;
};

extern "C" bool guardian_activate();
extern "C" void guardian_restart_required();

class Guardian final : public Mod {
public:
    ModMetaData GetMetaData() override {
        // These strings fit MSVC's inline string storage across CRTs.
        return {"Guardian", "Server Guardian", GUARDIAN_VERSION, "", "", false, true};
    }

    void Load(ModContext*) override { loaded = true; }
    void Unload(ModContext*) override { loaded = false; }
    void Activate(ModContext*) override { active = guardian_activate(); }
    void Deactivate(ModContext*) override {
        guardian_restart_required();
        active = false;
    }
    void Update(ModContext*) override {}
};

extern "C" Mod* guardian_shroudtopia_factory() {
    return new Guardian();
}
