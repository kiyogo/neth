#!/usr/bin/env python3
"""
Comprehensive Audit & Parity Tool for Minecraft Java blocks vs Rust Neth blocks.
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

def parse_block_ids_java():
    path = os.path.join(JVMC_PATH, "src", "net", "minecraft", "references", "BlockIds.java")
    mapping = {} # CONST_NAME -> block_id_str
    if not os.path.exists(path):
        return mapping
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            m = re.search(r'public\s+static\s+final\s+ResourceKey<Block>\s+([A-Z0-9_]+)\s*=\s*create\("([^"]+)"\)', line)
            if m:
                mapping[m.group(1)] = m.group(2)
    return mapping

def parse_block_item_ids_java():
    path = os.path.join(JVMC_PATH, "src", "net", "minecraft", "references", "BlockItemIds.java")
    mapping = {} # CONST_NAME -> block_id_str or list of str
    if not os.path.exists(path):
        return mapping

    with open(path, "r", encoding="utf-8") as f:
        content = f.read()

    # Simple creates: create("block_name") or create("block_name", "item_name")
    for m in re.finditer(r'public\s+static\s+final\s+BlockItemId\s+([A-Z0-9_]+)\s*=\s*BlockItemId\.create\("([^"]+)"', content):
        mapping[m.group(1)] = m.group(2)

    # Color collections: createSimpleColored("name")
    for m in re.finditer(r'public\s+static\s+final\s+ColorCollection<BlockItemId>\s+([A-Z0-9_]+)\s*=\s*createSimpleColored\("([^"]+)"\)', content):
        const_name = m.group(1)
        base_name = m.group(2)
        mapping[const_name] = [f"{color}_{base_name}" for color in DYE_COLORS]

    # Copper collections: createSimpleCopper("name")
    for m in re.finditer(r'public\s+static\s+final\s+WeatheringCopperCollection<BlockItemId>\s+([A-Z0-9_]+)\s*=\s*createSimpleCopper\("([^"]+)"\)', content):
        const_name = m.group(1)
        base_name = m.group(2)
        mapping[const_name] = [f"{prefix}{base_name}" for prefix in COPPER_PREFIXES]

    # Special copper block
    mapping["COPPER_BLOCK"] = [
        "copper_block", "exposed_copper", "weathered_copper", "oxidized_copper",
        "waxed_copper_block", "waxed_exposed_copper", "waxed_weathered_copper", "waxed_oxidized_copper"
    ]

    return mapping

def parse_jvmc_blocks():
    block_ids_map = parse_block_ids_java()
    block_item_ids_map = parse_block_item_ids_java()

    blocks_file = os.path.join(JVMC_PATH, "src", "net", "minecraft", "world", "level", "block", "Blocks.java")
    if not os.path.exists(blocks_file):
        print(f"Error: {blocks_file} does not exist")
        return {}

    with open(blocks_file, "r", encoding="utf-8") as f:
        content = f.read()

    # Registrations in Blocks.java:
    # 1. Single blocks:
    # public static final Block <FIELD> = register((BlockItemIds|BlockIds).<ID_CONST>, [Class::new], ...);
    # or register(<ID_CONST>, [Class::new], ...)
    # 2. ColorCollection:
    # public static final ColorCollection<Block> <FIELD> = ColorCollection.registerBlocks(BlockItemIds.<ID_CONST>, ..., ColorClass::new, ...)
    # or ColorCollection.zipMap(..., BlockItemIds.<ID_CONST>, (color, id) -> registerStair/registerSlab...)
    # 3. WeatheringCopperCollection:
    # WeatheringCopperCollection.registerBlocks(BlockItemIds.<ID_CONST>, ..., WaxedClass::new, WeatheringClass::new, ...)

    block_map = {} # block_id -> {field, java_class, ...}

    # Match single block registrations
    single_pattern = re.compile(
        r'public\s+static\s+final\s+([A-Za-z0-9_]+)\s+([A-Z0-9_]+)\s*=\s*register\(\s*(?:([A-Za-z0-9_]+)\.)?([A-Z0-9_]+)\s*(?:,\s*([A-Za-z0-9_]+)::new)?',
        re.MULTILINE
    )
    for m in single_pattern.finditer(content):
        field_type = m.group(1)
        field_name = m.group(2)
        id_source = m.group(3) or "BlockItemIds"
        id_const = m.group(4)
        ctor = m.group(5) or (field_type if field_type != "Block" else "Block")

        # Resolve block id
        if id_source == "BlockIds":
            resolved = block_ids_map.get(id_const, id_const.lower())
        else:
            resolved = block_item_ids_map.get(id_const, id_const.lower())

        if isinstance(resolved, list):
            for bid in resolved:
                block_map[bid] = {"field": field_name, "java_class": ctor}
        else:
            block_map[resolved] = {"field": field_name, "java_class": ctor}

    # Match ColorCollection registrations in Blocks.java
    color_reg_pattern = re.compile(
        r'public\s+static\s+final\s+ColorCollection<Block>\s+([A-Z0-9_]+)\s*=\s*ColorCollection\.registerBlocks\(\s*BlockItemIds\.([A-Z0-9_]+)\s*,\s*(?:[^\n]*?),\s*([A-Za-z0-9_]+)::new',
        re.MULTILINE
    )
    for m in color_reg_pattern.finditer(content):
        field_name = m.group(1)
        id_const = m.group(2)
        ctor = m.group(3)
        resolved_list = block_item_ids_map.get(id_const, [])
        if not resolved_list:
            resolved_list = [f"{color}_{field_name.lower()}" for color in DYE_COLORS]
        for bid in resolved_list:
            block_map[bid] = {"field": field_name, "java_class": ctor}

    # Match ColorCollection.zipMap with registerStair or registerSlab
    color_stair_pattern = re.compile(
        r'public\s+static\s+final\s+ColorCollection<Block>\s+([A-Z0-9_]+)\s*=\s*ColorCollection\.zipMap\(\s*ColorCollection\.VALUES,\s*BlockItemIds\.([A-Z0-9_]+),\s*\(color,\s*id\)\s*->\s*(registerStair|registerSlab)',
        re.MULTILINE
    )
    for m in color_stair_pattern.finditer(content):
        field_name = m.group(1)
        id_const = m.group(2)
        func = m.group(3)
        ctor = "StairBlock" if "Stair" in func else "SlabBlock"
        resolved_list = block_item_ids_map.get(id_const, [])
        for bid in resolved_list:
            block_map[bid] = {"field": field_name, "java_class": ctor}

    # Match WeatheringCopperCollection.registerBlocks
    copper_reg_pattern = re.compile(
        r'WeatheringCopperCollection\.registerBlocks\(\s*BlockItemIds\.([A-Z0-9_]+)\s*,\s*(?:[^\n]*?),\s*([A-Za-z0-9_]+)::new,\s*([A-Za-z0-9_]+)::new',
        re.MULTILINE
    )
    for m in copper_reg_pattern.finditer(content):
        id_const = m.group(1)
        waxed_ctor = m.group(2)
        weathering_ctor = m.group(3)
        resolved_list = block_item_ids_map.get(id_const, [])
        for bid in resolved_list:
            ctor = waxed_ctor if bid.startswith("waxed_") else weathering_ctor
            block_map[bid] = {"field": id_const, "java_class": ctor}

    # Match special WALL_BANNER and DYED_CANDLE_CAKE
    if "WALL_BANNER" not in block_map:
        for c in DYE_COLORS:
            block_map[f"{c}_wall_banner"] = {"field": "WALL_BANNER", "java_class": "WallBannerBlock"}
    if "DYED_CANDLE_CAKE" not in block_map:
        for c in DYE_COLORS:
            block_map[f"{c}_candle_cake"] = {"field": "DYED_CANDLE_CAKE", "java_class": "CandleCakeBlock"}

    return block_map

def parse_block_tags():
    tag_file = os.path.join(NETH_PATH, "crates", "pumpkin-data", "src", "generated", "tag.rs")
    tags = {}
    if not os.path.exists(tag_file):
        return tags

    with open(tag_file, "r", encoding="utf-8") as f:
        content = f.read()

    # Find the section between `pub mod Block {` and `static BLOCK_TAGS`
    start_idx = content.find("pub mod Block {")
    end_idx = content.find("static BLOCK_TAGS", start_idx)
    if start_idx != -1 and end_idx != -1:
        block_section = content[start_idx:end_idx]
    else:
        block_section = content

    tag_pattern = re.compile(
        r'pub\s+const\s+[A-Za-z0-9_]+\s*:\s*Tag\s*=\s*\(\s*&\[(.*?)\]\s*,\s*&\[.*?\]\s*,\s*"([^"]+)"\s*,?\s*\);',
        re.DOTALL
    )
    for m in tag_pattern.finditer(block_section):
        names_raw = m.group(1)
        tag_name = m.group(2)
        names = re.findall(r'"([^"]+)"', names_raw)
        tags[tag_name] = names
        if tag_name.startswith("minecraft:"):
            tags[tag_name[len("minecraft:"):]] = names

    return tags

def parse_rust_behaviors(tags):
    registry_file = os.path.join(NETH_PATH, "crates", "pumpkin-core", "src", "block", "registry.rs")
    with open(registry_file, "r", encoding="utf-8") as f:
        reg_content = f.read()

    registered_structs = set()
    for m in re.finditer(r'manager\.register\(([A-Za-z0-9_]+)\);', reg_content):
        registered_structs.add(m.group(1))

    blocks_dir = os.path.join(NETH_PATH, "crates", "pumpkin-core", "src", "block", "blocks")
    struct_to_blocks = {}
    block_to_struct = {}
    struct_files = {}

    for root, dirs, files in os.walk(blocks_dir):
        for f in files:
            if not f.endswith(".rs") or f == "tests.rs":
                continue
            path = os.path.join(root, f)
            with open(path, "r", encoding="utf-8") as rf:
                code = rf.read()

            for m in re.finditer(r'#\[pumpkin_block\((.*?)\)\]\s*(?:pub\s+)?struct\s+([A-Za-z0-9_]+)', code, re.DOTALL):
                args_str = m.group(1)
                struct_name = m.group(2)
                struct_files[struct_name] = path
                b_ids = re.findall(r'"(?:minecraft:)?([^"]+)"', args_str)
                # Also check for Block::NAME or BlockId::NAME
                for id_match in re.finditer(r'(?:Block|BlockId)::([A-Z0-9_]+)', args_str):
                    b_ids.append(id_match.group(1).lower())

                struct_to_blocks.setdefault(struct_name, set()).update(b_ids)
                for b in b_ids:
                    block_to_struct[b] = struct_name

            for m in re.finditer(r'#\[pumpkin_block_from_tag\("([^"]+)"\)\]\s*(?:pub\s+)?struct\s+([A-Za-z0-9_]+)', code):
                tag_name = m.group(1)
                struct_name = m.group(2)
                struct_files[struct_name] = path
                tag_blocks = tags.get(tag_name, [])
                struct_to_blocks.setdefault(struct_name, set()).update(tag_blocks)
                for b in tag_blocks:
                    block_to_struct[b] = struct_name

            for m in re.finditer(r'impl\s+(?:crate::block::)?BlockMetadata\s+for\s+([A-Za-z0-9_]+)\s*\{([\s\S]*?)\}', code):
                struct_name = m.group(1)
                struct_files[struct_name] = path
                body = m.group(2)
                b_ids = []
                for b_match in re.finditer(r'BlockId::([A-Z0-9_]+)', body):
                    b_ids.append(b_match.group(1).lower())
                if struct_name.startswith("Coral"):
                    for b_match in re.finditer(r'BlockId::([A-Z0-9_]+)', code):
                        b_ids.append(b_match.group(1).lower())
                for tag_name in re.findall(r'"(minecraft:[a-z0-9_]+)"', body):
                    b_ids.extend(tags.get(tag_name, []))
                struct_to_blocks.setdefault(struct_name, set()).update(b_ids)
                for b in b_ids:
                    block_to_struct[b] = struct_name

    return registered_structs, struct_to_blocks, block_to_struct, struct_files

def parse_pumpkin_data_blocks():
    blocks_rs = os.path.join(NETH_PATH, "crates", "pumpkin-data", "src", "generated", "block.rs")
    blocks = {}
    pattern = re.compile(r'pub\s+const\s+([A-Z0-9_]+):\s*Self\s*=\s*BlockId::new\((\d+)u16\)\.unwrap\(\);')
    with open(blocks_rs, "r", encoding="utf-8") as f:
        for line in f:
            m = pattern.search(line)
            if m:
                const_name = m.group(1)
                raw_id = int(m.group(2))
                name = const_name.lower()
                blocks[name] = {
                    "const": const_name,
                    "id": raw_id,
                    "name": name
                }
    return blocks

if __name__ == "__main__":
    jvmc_blocks = parse_jvmc_blocks()
    tags = parse_block_tags()
    reg_structs, struct_to_blocks, block_to_struct, struct_files = parse_rust_behaviors(tags)
    pumpkin_blocks = parse_pumpkin_data_blocks()

    print(f"=== BLOCK AUDIT SUMMARY ===")
    print(f"Total vanilla blocks registered in pumpkin-data: {len(pumpkin_blocks)}")
    print(f"Total Java blocks parsed from Blocks.java: {len(jvmc_blocks)}")
    print(f"Total registered structs in default_registry(): {len(reg_structs)}")
    print(f"Total block tags parsed: {len(tags)}")

    covered_blocks = set()
    for b_name in pumpkin_blocks:
        if b_name in block_to_struct and block_to_struct[b_name] in reg_structs:
            covered_blocks.add(b_name)

    print(f"Blocks in pumpkin-data covered by registered behaviors: {len(covered_blocks)} / {len(pumpkin_blocks)} ({len(covered_blocks)/len(pumpkin_blocks)*100:.2f}%)")

    uncovered = set(pumpkin_blocks.keys()) - covered_blocks
    java_class_uncovered = {}
    for b in uncovered:
        jc = jvmc_blocks.get(b, {}).get("java_class", "Block (Simple/Default)")
        java_class_uncovered.setdefault(jc, []).append(b)

    print(f"\nUncovered blocks count: {len(uncovered)}")
    print("\nBreakdown of uncovered blocks by Java class:")
    for jc, blist in sorted(java_class_uncovered.items(), key=lambda x: len(x[1]), reverse=True):
        print(f"  {jc:30s}: {len(blist):3d} blocks (e.g. {blist[:4]})")
