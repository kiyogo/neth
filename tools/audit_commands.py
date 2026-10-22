import os
import re

JVMC_PATH = r"E:\DATA\source\svmc\ServerMiecraft\jvmc"
NETH_PATH = r"E:\DATA\source\svmc\ServerMiecraft\Neth"

def get_jvmc_commands():
    commands_java = os.path.join(JVMC_PATH, "src", "net", "minecraft", "commands", "Commands.java")
    with open(commands_java, "r", encoding="utf-8") as f:
        content = f.read()

    # Find registered classes
    reg_calls = re.findall(r'([A-Za-z0-9_]+)\.register\(\s*this\.dispatcher', content)
    
    server_commands_dir = os.path.join(JVMC_PATH, "src", "net", "minecraft", "server", "commands")
    gametest_dir = os.path.join(JVMC_PATH, "src", "net", "minecraft", "gametest", "framework")
    
    command_info = {}
    
    for cls in reg_calls:
        found_file = None
        for root, dirs, files in os.walk(server_commands_dir):
            if f"{cls}.java" in files:
                found_file = os.path.join(root, f"{cls}.java")
                break
        if not found_file and cls == "TestCommand":
            candidate = os.path.join(gametest_dir, "TestCommand.java")
            if os.path.exists(candidate):
                found_file = candidate
                
        roots = []
        if found_file:
            with open(found_file, "r", encoding="utf-8") as f:
                code = f.read()
            # Find dispatcher.register(...literal("...")...)
            # Pattern looking for dispatcher.register(...) block
            for m in re.finditer(r'dispatcher\.register\s*\(\s*(?:[^\;]*?)(?:Commands|LiteralArgumentBuilder)\.literal\s*\(\s*"([^"]+)"\s*\)', code, re.DOTALL):
                roots.append(m.group(1))
            if not roots:
                # Sometimes dispatcher.register is formatted with casts or multiple lines
                for m in re.finditer(r'dispatcher\.register\s*\([\s\S]*?\.literal\s*\(\s*"([^"]+)"\s*\)', code):
                    roots.append(m.group(1))
                    break
            if not roots:
                # Fallback to literals in register method
                reg_method_m = re.search(r'public\s+static.*?void\s+register\s*\([^)]*\)\s*\{([\s\S]*?)\n\s{3,4}\}', code)
                if reg_method_m:
                    body = reg_method_m.group(1)
                    lits = re.findall(r'literal\s*\(\s*"([^"]+)"\s*\)', body)
                    if lits:
                        roots.append(lits[0])
        
        command_info[cls] = {
            "file": os.path.relpath(found_file, JVMC_PATH) if found_file else None,
            "roots": list(dict.fromkeys(roots))
        }
            
    return command_info

def get_neth_commands():
    mod_rs = os.path.join(NETH_PATH, "crates", "pumpkin-core", "src", "command", "commands", "mod.rs")
    with open(mod_rs, "r", encoding="utf-8") as f:
        mod_content = f.read()

    # Find modules registered in default_dispatcher
    registered_modules = set()
    for m in re.finditer(r'([a-z0-9_#]+)::register\(&mut dispatcher', mod_content):
        mod_name = m.group(1).replace("r#", "")
        registered_modules.add(mod_name)

    commands_dir = os.path.join(NETH_PATH, "crates", "pumpkin-core", "src", "command", "commands")
    neth_cmds = {}
    
    for f in os.listdir(commands_dir):
        if not f.endswith(".rs") or f in ["mod.rs", "tests.rs"]:
            continue
        mod_name = f[:-3]
        filepath = os.path.join(commands_dir, f)
        with open(filepath, "r", encoding="utf-8") as rf:
            code = rf.read()
            
        # Find command names / literals registered
        # Look for dispatcher.register(literal("...")), or CommandTree::new("...", ...) or Command::new("...")
        # or .name("...")
        roots = []
        for m in re.finditer(r'command\s*\(\s*"([^"]+)"', code):
            roots.append(m.group(1))
        for m in re.finditer(r'register_with_aliases\s*\([^,]+,\s*&\[(.*?)\]\)', code):
            roots.extend(re.findall(r'"([^"]+)"', m.group(1)))
        for m in re.finditer(r'const\s+NAMES\s*:\s*\[&str;\s*\d+\]\s*=\s*\[(.*?)\];', code):
            roots.extend(re.findall(r'"([^"]+)"', m.group(1)))
        for m in re.finditer(r'CommandTree::new\s*\(\s*&\[(.*?)\]', code):
            names_raw = m.group(1)
            roots.extend(re.findall(r'"([^"]+)"', names_raw))
        for m in re.finditer(r'Command::new\s*\(\s*&\[(.*?)\]', code):
            names_raw = m.group(1)
            roots.extend(re.findall(r'"([^"]+)"', names_raw))
        for m in re.finditer(r'literal\s*\(\s*"([^"]+)"\s*\)', code):
            roots.append(m.group(1))
        for m in re.finditer(r'name:\s*"([^"]+)"', code):
            roots.append(m.group(1))
            
        roots = list(dict.fromkeys(roots))
        primary_root = roots[0] if roots else mod_name
        
        # Check if implemented (has execute or run logic)
        is_implemented = bool(re.search(r'\.execute\s*\(|\.executes\s*\(|fn\s+execute|run\(', code))
        
        neth_cmds[mod_name] = {
            "file": os.path.relpath(filepath, NETH_PATH),
            "is_registered": mod_name in registered_modules,
            "roots": roots,
            "primary": primary_root,
            "is_implemented": is_implemented
        }
        
    return neth_cmds

def audit():
    jvmc_cmds = get_jvmc_commands()
    neth_cmds = get_neth_commands()

    # Build mapping from root name to Java class
    root_to_java = {}
    for cls, info in jvmc_cmds.items():
        for r in info["roots"]:
            root_to_java[r] = cls

    # Map root name to Neth command
    root_to_neth = {}
    for mod_name, info in neth_cmds.items():
        for r in info["roots"]:
            root_to_neth[r] = mod_name
        root_to_neth[info["primary"]] = mod_name
        root_to_neth[mod_name] = mod_name

    print("=== AUDIT COMPARISON: JAVA VS NETH COMMANDS ===")
    print(f"Total Java Command Classes: {len(jvmc_cmds)}")
    print(f"Total Neth Command Modules in commands/: {len(neth_cmds)}")
    
    # Check each Java command
    java_unique_roots = sorted(list(root_to_java.keys()))
    print(f"Total Distinct Root Commands in Java: {len(java_unique_roots)}")

    missing_in_neth = []
    registered_in_neth = []
    unregistered_in_neth = []

    for r in java_unique_roots:
        cls = root_to_java[r]
        if r in root_to_neth:
            mod = root_to_neth[r]
            info = neth_cmds[mod]
            if info["is_registered"]:
                registered_in_neth.append((r, cls, mod, info))
            else:
                unregistered_in_neth.append((r, cls, mod, info))
        else:
            missing_in_neth.append((r, cls))

    print(f"\nRegistered and active in Neth: {len(registered_in_neth)}")
    print(f"In Neth code but UNREGISTERED in default_dispatcher: {len(unregistered_in_neth)}")
    for r, cls, mod, info in unregistered_in_neth:
        print(f"  - /{r} (Java: {cls}, Neth module: {mod})")

    print(f"\nCompletely MISSING in Neth: {len(missing_in_neth)}")
    for r, cls in missing_in_neth:
        print(f"  - /{r} (Java: {cls})")

if __name__ == "__main__":
    audit()
