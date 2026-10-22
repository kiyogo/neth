import os
import re

JVMC_PATH = r"E:\DATA\source\svmc\ServerMiecraft\jvmc"
commands_java = os.path.join(JVMC_PATH, "src", "net", "minecraft", "commands", "Commands.java")

with open(commands_java, "r", encoding="utf-8") as f:
    lines = f.readlines()

standard = []
dedicated = []
integrated = []
debug_dev = []

curr_section = standard

for line in lines:
    if "if (JvmProfiler.INSTANCE.isAvailable())" in line or "if (SharedConstants.DEBUG_CHASE_COMMAND)" in line or "if (SharedConstants.DEBUG_DEV_COMMANDS" in line:
        curr_section = debug_dev
    elif "if (commandSelection.includeDedicated)" in line:
        curr_section = dedicated
    elif "if (commandSelection.includeIntegrated)" in line:
        curr_section = integrated
    elif "this.dispatcher.setConsumer" in line:
        break
        
    m = re.search(r'([A-Za-z0-9_]+)\.register\(\s*this\.dispatcher', line)
    if m:
        curr_section.append(m.group(1))

print(f"Standard Commands ({len(standard)}): {standard}")
print(f"Dedicated Commands ({len(dedicated)}): {dedicated}")
print(f"Integrated Commands ({len(integrated)}): {integrated}")
print(f"Debug/Dev Commands ({len(debug_dev)}): {debug_dev}")
print(f"Total Standard + Dedicated: {len(standard) + len(dedicated)}")
