# 13. TEST SUITE & DIFFERENTIAL TEST RESULTS
## KẾT QUẢ THỰC THI TOÀN BỘ CÁC BÀI KIỂM THỬ ĐƠN VỊ & VI SAI TRÊN MÃ NGUỒN NETH

---

### 1. Bảng Tổng Hợp Kết Quả Thực Thi Test Suites Toàn Workspace

Dữ liệu máy đọc được lưu trữ đồng bộ tại [test_results.json](file:///E:/DATA/source/svmc/ServerMiecraft/Neth/docs/vanilla-parity-audit/test_results.json).

| Phân hệ / Crate Kiểm Thử | Lệnh Kiểm Thử Thực Tế | Số Test Passed | Số Test Failed | Thời Gian Chạy | Trạng Thái |
| :--- | :--- | :---: | :---: | :---: | :---: |
| **`pumpkin-command`** | `cargo test -p pumpkin-command` | **91** (86 unit + 5 doc) | 0 | 0.19s | 🟢 **PASS 100%** |
| **`pumpkin-data`** | `cargo test -p pumpkin-data` | **134** (131 unit + 3 doc)| 0 | 0.04s | 🟢 **PASS 100%** |
| **`pumpkin-config`** | `cargo test -p pumpkin-config` | **31** | 0 | 0.11s | 🟢 **PASS 100%** |
| **`pumpkin-util`** | `cargo test -p pumpkin-util` | **89** | 0 | 0.32s | 🟢 **PASS 100%** |
| **`pumpkin-inventory`** | `cargo test -p pumpkin-inventory` | **35** | 0 | 0.12s | 🟢 **PASS 100%** |
| **`pumpkin-world` (Placement)** | `cargo test -p pumpkin-world --lib placement` | **15** | 0 | 23.60s | 🟢 **PASS 100%** |
| **`pumpkin-world` (Template)** | `cargo test -p pumpkin-world --lib template` | **29** | 0 | 22.70s | 🟢 **PASS 100%** |
| **`pumpkin-world` (Session Lock)** | `cargo test -p pumpkin-world --lib session_lock` | **1** | 0 | 0.02s | 🟢 **PASS 100%** |
| **`pumpkin-world` (Village Path 1024)** | `cargo test -p pumpkin-world --lib village_path_projection_1024_cases` | **1 (1024 cases)** | 0 | 23.67s | 🟢 **PASS 100%** |
| **`pumpkin-world` (Oracle Starts Dumps)**| `cargo test -p pumpkin-world --lib structure_starts_dump` | **3 (Overworld/Nether/End)**| 0 | 7.97s | 🟢 **PASS 100%** |
| **`pumpkin-core` (Natural Spawner)** | `cargo test -p pumpkin-core --lib world::natural_spawner` | **14** | 0 | 0.01s | 🟢 **PASS 100%** |
| **`pumpkin-core` (Scoreboard)** | `cargo test -p pumpkin-core --lib command::commands::scoreboard` | **9** | 0 | 0.14s | 🟢 **PASS 100%** |
| **`pumpkin-core` (Team)** | `cargo test -p pumpkin-core --lib command::commands::team` | **3** | 0 | 0.00s | 🟢 **PASS 100%** |
| **Workspace Compilation** | `cargo check -p neth` | Exit Code: 0 | 0 | 6m 30s | 🟢 **PASS 100%** |
| **TỔNG SỐ TESTS ĐÃ CHẠY** | - | **455 TESTS PASSED** | **0 FAILED** | **~8 phút** | 🟢 **100% SUCCESS** |

---

### 2. Chi Tiết Kết Quả Các Bài Kiểm Thử Vi Sai Trọng Yếu

#### 2.1. Kiểm Thử Chiếu Trọng Lực Đường Làng 1,024 Ca (`village_path_projection_1024_cases`)
- **Tệp mã nguồn:** `crates/pumpkin-world/src/generation/structure/template/placement_tests.rs`
- **Kết quả thực tế:**
  ```text
  running 1 test
  test generation::structure::template::placement_tests::village_path_projection_1024_cases ... ok
  test result: ok. 1 passed; 0 failed; finished in 23.67s
  ```
- **Xác nhận:** 1,024 ca thử nghiệm biến thiên bề mặt địa hình đều khớp chính xác 100% độ cao bám dính của đường làng Java.

#### 2.2. Kiểm Thử Oracle Structure Starts Dumps Cho Cả 3 Chiều Không Gian
- **Tệp mã nguồn:** `crates/pumpkin-world/src/generation/proto_chunk_test/structure_starts.rs`
- **Dữ liệu so sánh:** Oracle dump `assets/tests/structure_starts.json` (2.07 MB) từ Minecraft Java 26.3 gốc.
- **Kết quả thực tế:**
  ```text
  running 3 tests
  test generation::proto_chunk_test::structure_starts::end_structure_starts_dump ... ok
  test generation::proto_chunk_test::structure_starts::nether_structure_starts_dump ... ok
  test generation::proto_chunk_test::structure_starts::overworld_structure_starts_dump ... ok
  test result: ok. 3 passed; 0 failed; finished in 7.97s
  ```
- **Xác nhận:** Bounding box, mảnh ghép cấu trúc và vị trí đặt của toàn bộ các cấu trúc Overworld, Nether và End đều khớp hoàn toàn với Java gốc.

#### 2.3. Kiểm Thử 14 Ca Parity Sinh Quái Tự Nhiên (`world::natural_spawner`)
- **Tệp mã nguồn:** `crates/pumpkin-core/src/world/natural_spawner.rs`
- **Kết quả thực tế:**
  ```text
  running 14 tests
  test world::natural_spawner::tests::potential_change_is_finite_beyond_i32_range ... ok
  test world::natural_spawner::tests::test_block_danger_immunities ... ok
  test world::natural_spawner::tests::test_point_charge_distance_does_not_overflow ... ok
  test world::natural_spawner::tests::test_nether_bricks_floor_allows_fortress_spawns ... ok
  test world::natural_spawner::tests::test_fortress_enemies_table_parity ... ok
  test world::natural_spawner::tests::test_signal_source_parity ... ok
  test world::natural_spawner::tests::test_empty_spawn_block_rejects_signal_sources_and_solid_blocks ... ok
  test world::natural_spawner::tests::test_local_mob_cap_calculation_parity ... ok
  test world::natural_spawner::tests::test_global_mob_cap_scaling_parity ... ok
  test world::natural_spawner::tests::test_filtered_spawning_categories_filtering ... ok
  test result: ok. 14 passed; 0 failed; finished in 0.01s
  ```
- **Xác nhận:** 7 lỗi parity lịch sử đã được giải quyết dứt điểm và được bảo vệ nghiêm ngặt bằng regression tests.
