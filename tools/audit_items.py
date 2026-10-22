#!/usr/bin/env python3
"""
Comprehensive Audit & Parity Tool for Minecraft Java Items vs Rust Neth Items.
"""

import os
import re
import json

JVMC_PATH = r"E:\DATA\source\svmc\ServerMiecraft\jvmc"
NETH_PATH = r"E:\DATA\source\svmc\ServerMiecraft\Neth"

DYE_COLORS = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
    "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"
]

COPPER_PREFIXES = [
    "", "exposed_", "weathered_", "oxidized_",
    "waxed_", "waxed_exposed_", "waxed_weathered_", "waxed_oxidized_"
]

def parse_item_ids_java():
    path = os.path.join(JVMC_PATH, "src", "net", "minecraft", "references", "ItemIds.java")
    mapping = {}
    if not os.path.exists(path):
        return mapping
    with open(path, "r", encoding="utf-8") as f:
        content = f.read()

    # Direct create("name")
    for m in re.finditer(r'public\s+static\s+final\s+ResourceKey<Item>\s+([A-Z0-9_]+)\s*=\s*create\("([^"]+)"\)', content):
        mapping[m.group(1)] = m.group(2)

    # createSpawnEgg(EntityTypeIds.FOO)
    for m in re.finditer(r'public\s+static\s+final\s+ResourceKey<Item>\s+([A-Z0-9_]+)\s*=\s*createSpawnEgg\(EntityTypeIds\.([A-Z0-9_]+)\)', content):
        const_name = m.group(1)
        entity_name = m.group(2).lower()
        mapping[const_name] = f"{entity_name}_spawn_egg"

    # createMusicDisc(JukeboxSongs.FOO)
    disc_name_map = {
        "THIRTEEN": "13",
        "CAT": "cat",
        "BLOCKS": "blocks",
        "CHIRP": "chirp",
        "FAR": "far",
        "MALL": "mall",
        "MELLOHI": "mellohi",
        "STAL": "stal",
        "STRAD": "strad",
        "WARD": "ward",
        "ELEVEN": "11",
        "WAIT": "wait",
        "PIGSTEP": "pigstep",
        "OTHERSIDE": "otherside",
        "FIVE": "5",
        "RELIC": "relic",
        "PRECIPICE": "precipice",
        "CREATOR": "creator",
        "CREATOR_MUSIC_BOX": "creator_music_box",
        "TEARS": "tears",
        "LAVA_CHICKEN": "lava_chicken",
        "BOUNCE": "bounce",
    }
    for m in re.finditer(r'public\s+static\s+final\s+ResourceKey<Item>\s+([A-Z0-9_]+)\s*=\s*createMusicDisc\(JukeboxSongs\.([A-Z0-9_]+)\)', content):
        const_name = m.group(1)
        song_const = m.group(2)
        song_name = disc_name_map.get(song_const, song_const.lower())
        mapping[const_name] = f"music_disc_{song_name}"

    # createArmorTrimSmithingTemplate(TrimPatterns.FOO)
    for m in re.finditer(r'public\s+static\s+final\s+ResourceKey<Item>\s+([A-Z0-9_]+)\s*=\s*createArmorTrimSmithingTemplate\(TrimPatterns\.([A-Z0-9_]+)\)', content):
        const_name = m.group(1)
        pattern_name = m.group(2).lower()
        mapping[const_name] = f"{pattern_name}_armor_trim_smithing_template"

    # createPotterySherd(DecoratedPotPatterns.FOO)
    for m in re.finditer(r'public\s+static\s+final\s+ResourceKey<Item>\s+([A-Z0-9_]+)\s*=\s*createPotterySherd\(DecoratedPotPatterns\.([A-Z0-9_]+)\)', content):
        const_name = m.group(1)
        pattern_name = m.group(2).lower()
        mapping[const_name] = f"{pattern_name}_pottery_sherd"

    # Color collections: createSimpleColored("name")
    for m in re.finditer(r'public\s+static\s+final\s+ColorCollection<ResourceKey<Item>>\s+([A-Z0-9_]+)\s*=\s*createSimpleColored\("([^"]+)"\)', content):
        const_name = m.group(1)
        base_name = m.group(2)
        for color in DYE_COLORS:
            mapping[f"{color.upper()}_{const_name}"] = f"{color}_{base_name}"

    return mapping

def parse_block_item_ids_java():
    path = os.path.join(JVMC_PATH, "src", "net", "minecraft", "references", "BlockItemIds.java")
    mapping = {}
    if not os.path.exists(path):
        return mapping

    with open(path, "r", encoding="utf-8") as f:
        content = f.read()

    # Simple creates: create("block_name") or create("block_name", "item_name")
    for m in re.finditer(r'public\s+static\s+final\s+BlockItemId\s+([A-Z0-9_]+)\s*=\s*BlockItemId\.create\("([^"]+)"(?:,\s*"([^"]+)")?\)', content):
        const_name = m.group(1)
        item_name = m.group(3) if m.group(3) else m.group(2)
        mapping[const_name] = item_name

    # Color collections: createSimpleColored("name")
    for m in re.finditer(r'public\s+static\s+final\s+ColorCollection<BlockItemId>\s+([A-Z0-9_]+)\s*=\s*createSimpleColored\("([^"]+)"\)', content):
        const_name = m.group(1)
        base_name = m.group(2)
        for color in DYE_COLORS:
            mapping[f"{color.upper()}_{const_name}"] = f"{color}_{base_name}"

    # Copper collections: createSimpleCopper("name")
    for m in re.finditer(r'public\s+static\s+final\s+WeatheringCopperCollection<BlockItemId>\s+([A-Z0-9_]+)\s*=\s*createSimpleCopper\("([^"]+)"\)', content):
        const_name = m.group(1)
        base_name = m.group(2)
        for prefix in COPPER_PREFIXES:
            c_name = f"{prefix.upper()}{const_name}"
            mapping[c_name] = f"{prefix}{base_name}"

    # Special copper block
    special_copper = {
        "": "copper_block",
        "exposed_": "exposed_copper",
        "weathered_": "weathered_copper",
        "oxidized_": "oxidized_copper",
        "waxed_": "waxed_copper_block",
        "waxed_exposed_": "waxed_exposed_copper",
        "waxed_weathered_": "waxed_weathered_copper",
        "waxed_oxidized_": "waxed_oxidized_copper",
    }
    for prefix, full_name in special_copper.items():
        mapping[f"{prefix.upper()}COPPER_BLOCK"] = full_name

    return mapping

def parse_rust_items():
    item_rs = os.path.join(NETH_PATH, "crates", "pumpkin-data", "src", "generated", "item.rs")
    rust_items = {}
    if not os.path.exists(item_rs):
        return rust_items
    
    with open(item_rs, "r", encoding="utf-8") as f:
        content = f.read()
    
    # Find within `impl Item { ... }`
    impl_match = re.search(r'impl Item \{(.*?)\nimpl', content, re.DOTALL)
    if not impl_match:
        return rust_items
    
    impl_content = impl_match.group(1)
    for m in re.finditer(r'pub const ([A-Z0-9_]+): Self = Self\s*\{[^}]*id:\s*(\d+)[^}]*registry_key:\s*"([^"]+)"', impl_content):
        const_name = m.group(1)
        item_id = int(m.group(2))
        registry_key = m.group(3)
        rust_items[registry_key] = {
            "const_name": const_name,
            "id": item_id,
            "registry_key": registry_key
        }
    return rust_items

def parse_rust_registered_behaviors():
    mod_rs = os.path.join(NETH_PATH, "crates", "pumpkin-core", "src", "item", "items", "mod.rs")
    behaviors = set()
    if not os.path.exists(mod_rs):
        return behaviors
    with open(mod_rs, "r", encoding="utf-8") as f:
        for line in f:
            m = re.search(r'manager\.register\(([A-Za-z0-9_]+)\)', line)
            if m:
                behaviors.add(m.group(1))
    return behaviors

def main():
    item_ids = parse_item_ids_java()
    block_item_ids = parse_block_item_ids_java()
    all_java_items = {}
    for k, v in item_ids.items():
        all_java_items[v] = f"ItemIds.{k}"
    for k, v in block_item_ids.items():
        all_java_items[v] = f"BlockItemIds.{k}"

    print(f"Java ItemIds count: {len(item_ids)}")
    print(f"Java BlockItemIds count: {len(block_item_ids)}")
    print(f"Total Unique Java Items: {len(all_java_items)}")

    rust_items = parse_rust_items()
    print(f"Rust Generated Items in impl Item: {len(rust_items)}")

    registered_behaviors = parse_rust_registered_behaviors()
    print(f"Rust Item Behaviors Registered: {len(registered_behaviors)}")

    # Check differences
    missing_in_rust = set(all_java_items.keys()) - set(rust_items.keys())
    extra_in_rust = set(rust_items.keys()) - set(all_java_items.keys())
    print(f"Missing in Rust: {len(missing_in_rust)}")
    if missing_in_rust:
        print("Sample missing in Rust:", list(missing_in_rust)[:10])
    print(f"Extra in Rust: {len(extra_in_rust)}")
    if extra_in_rust:
        print("Sample extra in Rust:", list(extra_in_rust)[:10])

if __name__ == "__main__":
    main()
