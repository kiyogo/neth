# Neth

Neth is a high-performance Minecraft server engine engineered entirely in Rust. Designed from the ground up for concurrent execution, strict protocol compliance, and resource efficiency, the project delivers a modern server architecture capable of handling intensive multiplayer workloads with minimal latency.

The project is developed and maintained by KIYOVN.

Website: https://kiyomc.com  
Repository: https://github.com/kiyogo/neth  

---

## Overview

Minecraft server hosting has historically relied on single-threaded execution models, leading to performance bottlenecks during entity ticking, lighting calculation, and chunk generation. Neth addresses these architectural limitations by taking full advantage of Rust's fearless concurrency and memory safety guarantees.

Through a multi-threaded scheduling architecture, Neth offloads chunk generation and illumination tasks to parallel worker threads, while preserving synchronous deterministic state handling for core gameplay events. This approach ensures high tick stability, low memory footprints, and responsive client synchronization across demanding runtime conditions.

Neth is an independent fork of the Pumpkin server project (https://github.com/Pumpkin-MC/Pumpkin), incorporating extensive parity enhancements, architectural refinements, and gameplay parity implementations.

---

## Project Status

- Current Release Version: 0.2.0
- Target Minecraft Java Version: 26.3
- Target Protocol Version: 777
- Target World Data Version: 5023
- Current Implementation Status: 90% Completion Milestone

The engine has completed 90% of its target parity roadmap against reference vanilla server mechanics. Implemented subsystems include complete Brigadier command graph resolution, Anvil chunk persistence, multi-noise world generation, redstone logic propagation, inventory transaction security, and cross-platform network protocol handling.

---

## Architectural Principles

### Parallel Workload Distribution

CPU-intensive computational workloads are segregated across specialized execution pools. World terrain synthesis, noise generation, and bi-directional lighting propagation utilize asynchronous worker threads. The main tick scheduler coordinates game ticks with sub-millisecond precision, preventing tick lag from blocking packet processing.

### Protocol and Cross-Platform Networking

Neth supports modern Java Edition protocol pipelines alongside Bedrock Edition networking via NetherNet integration. Communication streams employ hardware-accelerated encryption, zlib compression, and zero-allocation packet parsing to minimize serialization overhead under heavy connection volume.

### Storage Safety and Durability

World data storage adheres strictly to standard Anvil region formats with multi-tier fail-safe persistence. File rotations, atomic temporary replacements, and forensic backup rotation safeguard player profiles and level states against unexpected termination or hardware failures.

### Extensible Runtime Subsystems

The server includes modular extension points through both a native plugin interface and a WebAssembly (WASM) host runtime built on Wasmtime. This sandbox enables third-party developers to extend server logic with language neutrality and near-native runtime speed.

---

## Community and Governance

Neth is developed as a collaborative open-source project. Contributors and community members are expected to uphold respectful, constructive interactions as detailed in the project Code of Conduct. Information regarding contribution procedures and vulnerability disclosures is maintained in the repository root documents.

---

## Licensing and Copyright

Neth is free and open-source software licensed under the GNU General Public License version 3.0 (GPL-3.0).

Portions of this codebase originate from the Pumpkin project, Copyright 2024-2026 Aleksander Medvedev and the Pumpkin contributors, licensed under the GNU General Public License version 3.0. Modifications, parity systems, and architectural enhancements are Copyright 2026 KIYOVN.
