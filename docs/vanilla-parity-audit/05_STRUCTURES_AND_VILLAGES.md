# 05. STRUCTURES & VILLAGE PARITY AUDIT
## ĐỐI CHIẾU CÔNG TRÌNH & HỆ THỐNG LÀNG MINECRAFT 26.3

---

### 1. Tổng quan Ma trận Công trình (52 Structures Matrix)

Neth hỗ trợ hai cơ chế sinh công trình:
1. **Jigsaw Structures (Sinh theo Template Pool):** Bao gồm Làng (Plains, Desert, Savanna, Snowy, Taiga), Pillager Outpost, Ancient City, Bastion Remnant, Trail Ruins, Trial Chambers.
2. **Procedural Structures (Sinh theo Thuật toán Thủ tục):** Stronghold (128 vị trí vòng đồng tâm), Mineshaft, Ocean Monument, Nether Fortress, End City, Buried Treasure.

---

### 2. Điều tra Chuyên sâu: Nguyên Nhân Gốc Rễ Lỗi Làng Không Có Dân Làng

Trước khi được phân tích và khắc phục trong các bản cập nhật gần đây, Neth gặp lỗi nghiêm trọng: **Các ngôi làng sinh ra hoàn chỉnh với nhà cửa, đường sá nhưng tuyệt đối không có bất kỳ Dân Làng (Villager) hay Golem nào**.

#### Phân tích Cơ chế Vanilla Java (`jvmc`):
Trong Minecraft Java 26.3, Villagers **không nằm cố định trong tệp `.nbt` của từng ngôi nhà**. Thay vào đó:
- Mỗi ngôi nhà chứa các khối Jigsaw kết nối đến pool `minecraft:village/<biome>/villagers`.
- Pool này chứa các template kích thước 1x1x1 khối: `unemployed.nbt`, `nitwit.nbt`, `baby.nbt`.
- Bên trong compound tag `entities` của các file NBT nhỏ này chứa thông tin thực thể Villager với tọa độ tương đối.

#### Nguyên nhân Lỗi trong Neth (Root Cause):
Trong file `crates/pumpkin-world/src/generation/structure/structures/jigsaw.rs`, mã nguồn trước đây đã áp dụng hàm biến đổi góc xoay `corner = self.rotation.rotate_offset(...)` và tính toán `placement_origin`. Sau đó, `placement_origin` này được truyền vào `place_template_entities`.
Bên trong `place_template_entities`, hàm lại tiếp tục gọi `rotation.transform_pos(...)` một lần nữa.
**Hậu quả:** Tọa độ thực thể bị cộng dồn offset hai lần, làm tọa độ thế giới của Dân Làng văng ra ngoài ranh giới `chunk_box` đang sinh. Câu lệnh điều kiện `if !chunk_box.contains_pos(&world_block_pos) { continue; }` đã âm thầm loại bỏ 100% Dân Làng!

#### Giải pháp Đã Triển khai (Verified Solution):
1. Chuẩn hóa hàm biến đổi tọa độ theo đúng công thức của Vanilla Java (`StructureTemplate.java:483-513`):
   - Sử dụng `StructureTemplate::transform_block_pos` và `StructureTemplate::transform_vec3` xoay quanh tâm trục `(0, 0, 0)`.
   - Truyền trực tiếp `origin` của mảnh ghép (`self.pos.0`).
2. Tích hợp cờ bảo toàn thực thể: Mọi thực thể sinh ra từ công trình được gán `persistence_required = true` tại `spawn_structure_entities` trong `pumpkin-core`, đảm bảo không bị despawn khi người chơi rời khỏi chunk.

---

### 3. Tương thích Địa hình & Đệm Móng Nhà (Beardifier Engine)

- **Kernel:** Neth sử dụng mảng tra cứu hạt nhân 24x24x24 (`BEARD_KERNEL`) với hàm suy giảm hàm mũ `e^(-dist^2 / 16)`, hoàn toàn trùng khớp với `Beardifier.java` trong `jvmc`.
- **Terrain Adaptation:**
  - `BEARD_THIN` (Hệ số 0.8): Áp dụng cho Nhà làng và Tháp Pillager để đệm móng đất/đá tự nhiên, ngăn nhà bị lơ lửng trên vực.
  - `BEARD_BOX`: Áp dụng cho Ancient City để khoét rỗng không gian ngầm.
  - `ENCAPSULATE`: Áp dụng cho Trial Chambers.

---

### 4. Chiếu Trọng lực Đường Làng (Street Projection Differential Test)

Đường làng được sinh với chế độ `JigsawProjection::TerrainMatching`. Đường sá phải uốn lượn theo bề mặt địa hình tự nhiên (`HeightmapType::WorldSurfaceWg`).
Neth đã vượt qua **1024 trường hợp kiểm thử vi sai tự động** (`village_path_projection_1024_cases` trong `crates/pumpkin-world/src/generation/structure/template/placement_tests.rs`), xác nhận độ chính xác 100% so với Minecraft Java.

---

### 5. Hoàn Thiện & Xác Minh Toàn Bộ 52 Cấu Trúc Vanilla 26.3

#### 5.1. Khắc phục SinglePieceStructure (`desert_pyramid`, `jungle_pyramid`):
- Đối chiếu với `SinglePieceStructure.java:28-30`: Bổ sung kiểm tra độ cao 4 góc bounding box (`lowest_y < sea_level`). Nếu bất kỳ góc nào dưới mực nước biển (63), cấu trúc lập tức bị loại bỏ.
- Chuẩn hóa điểm neo bắt đầu (`start_pos`) về tâm chunk `(chunk_x * 16 + 8, center_y, chunk_z * 16 + 8)` để khớp vị trí kiểm tra biome của Vanilla.

#### 5.2. Chuẩn hóa Điểm Neo `onTopOfChunkCenter` (`swamp_hut`, `igloo`):
- Đã căn chỉnh tọa độ kiểm tra biome của Swamp Hut và Igloo về tâm chunk `(x + 8, surface_y, z + 8)` thay vì góc Tây Bắc hoặc độ cao cố định Y=64/90.

#### 5.3. Kết quả Kiểm thử Vi sai (Differential Oracle Testing):
- Toàn bộ 52 cấu trúc đã được ánh xạ chi tiết tại `docs/parity/structures/java-rust-mapping.md`.
- Vượt qua 100% bộ kiểm thử Oracle Java 26.3 (`assets/tests/structure_starts.json`) cho cả 3 chiều:
  - `overworld_structure_starts_dump`: **PASSED**
  - `nether_structure_starts_dump`: **PASSED**
  - `end_structure_starts_dump`: **PASSED**
- 100% các unit test chuyên biệt (66/66 tests) trong `pumpkin-world::generation::structure` đều **PASSED**.

