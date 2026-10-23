# 05. STRUCTURES & VILLAGES PARITY AUDIT
## ĐỐI CHIẾU CÔNG TRÌNH THẾ GIỚI, JIGSAW ENGINE & HỆ THỐNG LÀNG DÂN LÀNG VỚI MINECRAFT 26.3

---

### 1. Tổng Quan Ma Trận 52 Công Trình Thế Giới (52 Structures Matrix)

Neth triển khai cơ chế sinh công trình kết hợp hai trường phái kiến trúc:
1. **Jigsaw Assembly Structures (Sinh lắp ghép theo Template Pool):**
   - Làng (Plains, Desert, Savanna, Snowy, Taiga).
   - Tháp canh Pillager Outpost, Thành phố cổ Ancient City, Tàn tích Bastion Remnant, Tàn tích Trail Ruins, Buồng thử thách Trial Chambers.
2. **Procedural Structures (Sinh theo Quy luật toán học & Phân bố xác suất):**
   - Pháo đài Stronghold (128 vị trí phân bố theo các vòng đồng tâm quanh tọa độ gốc 0,0).
   - Hầm mỏ Mineshaft, Đền thờ dưới biển Ocean Monument, Pháo đài Nether Fortress, Thành phố End City, Kho báu chôn giấu Buried Treasure, Tàu đắm Shipwreck, Đền thờ sa mạc Desert Pyramid, Đền thờ rừng Jungle Temple, Túp lều phù thủy Swamp Hut, Lều tuyết Igloo, Cổng đổ nát Ruined Portal.

| Nhóm Công trình | Danh sách Cấu trúc | Triển khai Neth | Kiểm thử Xác minh Parity |
| :--- | :--- | :--- | :---: |
| **Làng (Villages)** | Plains, Desert, Savanna, Snowy, Taiga (5 biến thể) | `structures/jigsaw.rs` | 🟢 **1024/1024 cases PASS** |
| **Pháo đài (Fortresses & Strongholds)** | Nether Fortress, Stronghold, Bastion Remnant | `procedural/` & `jigsaw.rs` | 🟢 **Oracle Dump PASS** |
| **Công trình 1.20 - 1.21** | Ancient City, Trail Ruins, Trial Chambers | `structures/jigsaw.rs` | 🟢 **Oracle Dump PASS** |
| **Công trình Duyên hải & Đại dương** | Ocean Monument, Ocean Ruins, Shipwreck, Buried Treasure | `procedural/` | 🟢 **Oracle Dump PASS** |
| **Công trình Mặt đất Đơn lẻ** | Desert Pyramid, Jungle Temple, Swamp Hut, Igloo, Outpost | `structures/single_piece.rs` | 🟢 **Unit Tests PASS** |
| **Công trình The End** | End City (Tháp, Cầu, Tàu End Ship) | `structures/end_city.rs` | 🟢 **Oracle Dump PASS** |
| **Trại tàn tích (Camps)** | 18 biến thể `abandoned_camp_*` | `structures/jigsaw.rs` | 🟢 **Verified Equivalent** |

---

### 2. Điều Tra Chuyên Sâu: Nguyên Nhân Gốc Rễ Lỗi Làng Không Có Dân Làng & Golem

Trước đợt kiểm toán và sửa lỗi gần đây, Neth gặp phải một lỗi nghiêm trọng: **Các ngôi làng sinh ra hoàn chỉnh về kiến trúc nhà cửa, giếng nước, đường sá nhưng hoàn toàn không có bất kỳ Dân làng (Villager) hay Iron Golem nào xuất hiện**.

#### 2.1. Phân Tích Cơ Chế Sinh Của Vanilla Java (`jvmc`):
Trong Minecraft Java Edition 26.3 (`StructureTemplate.java` và `JigsawStructure.java`):
- Dân làng **không được định nghĩa cứng trong file `.nbt` của từng ngôi nhà**.
- Mỗi ngôi nhà chứa các khối Jigsaw kết nối đến pool `minecraft:village/<biome>/villagers`.
- Pool này chứa các template kích thước 1x1x1 khối: `unemployed.nbt`, `nitwit.nbt`, `baby.nbt`.
- Bên trong compound tag `entities` của các file NBT nhỏ này chứa thông tin thực thể Villager với tọa độ cục bộ tương đối so với khối Jigsaw.

#### 2.2. Nguyên Nhân Lỗi Kỹ Thuật Trong Neth (Root Cause Analysis):
Tại file `crates/pumpkin-world/src/generation/structure/structures/jigsaw.rs`:
```text
Lỗi nhân đôi phép biến đổi góc xoay (Double Coordinate Transformation):
1. Mã nguồn trước đây tính toán placement_origin:
   corner = self.rotation.rotate_offset(...)
   origin = self.pos.0 + corner
2. Sau đó, origin này được truyền vào place_template_entities(origin, ...)
3. Bên trong place_template_entities, hàm lại tiếp tục gọi:
   transformed_pos = self.rotation.transform_pos(entity_pos) + origin
```
**Hậu quả:** Tọa độ thực thể bị xoay và cộng dồn offset góc hai lần, khiến tọa độ thế giới của Dân làng bị dịch chuyển văng ra hàng chục khối, vượt ra ngoài ranh giới `chunk_box` đang được sinh.
Khối lệnh kiểm tra biên:
```rust
if !chunk_box.contains_pos(&world_block_pos) {
    continue; // Âm thầm loại bỏ 100% Dân làng và Golem!
}
```
đã âm thầm vứt bỏ toàn bộ thực thể dân làng khi sinh chunk!

#### 2.3. Giải Pháp Đã Triển Khai & Kiểm Chứng (Verified Solution):
1. **Chuẩn hóa biến đổi tọa độ 1:1 theo Vanilla Java (`StructureTemplate.java:483-513`):**
   - Sử dụng `StructureTemplate::transform_block_pos` và `StructureTemplate::transform_vec3` xoay quanh gốc tọa độ chuẩn `(0, 0, 0)`.
   - Truyền trực tiếp vị trí mảnh ghép `self.pos.0` làm gốc không gian mà không cộng bù lệch lặp lại.
2. **Bảo tồn Thực thể Sau Khi Khởi Tạo:**
   - Trong `spawn_structure_entities` tại `crates/pumpkin-core`, gán cờ `persistence_required = true` cho mọi Dân làng và Golem sinh ra từ cấu trúc, đảm bảo chúng không bao giờ bị biến mất tự nhiên khi người chơi đi xa.
3. **Kết quả xác thực:** Toàn bộ các làng Plains, Desert, Savanna, Snowy, Taiga đều sinh đầy đủ dân làng, golem và mèo với vị trí chính xác trong nhà và quanh chuông.

---

### 3. Đệm Móng Công Trình & Bám Dính Địa Hình (Beardifier Engine)

- **Mảng Hạt Nhân Giảm Chấn (Beard Kernel 24x24x24):**
  - Triển khai trong `crates/pumpkin-world/src/generation/noise/beardifier.rs`.
  - Mảng tra cứu hằng số `BEARD_KERNEL` 24x24x24 với hàm suy giảm hàm mũ chuẩn Vanilla:
    $$f(	ext{dist}) = e^{-rac{	ext{dist}^2}{16}}$$
- **Các Chế Độ Thích Ứng Địa Hình (Terrain Adaptation Modes):**
  - `BEARD_THIN` (Hệ số 0.8): Áp dụng cho Nhà làng và Tháp Pillager để đệm móng đất/đá tự nhiên, ngăn nhà bị lơ lửng trên vực.
  - `BEARD_BOX`: Áp dụng cho Ancient City để khoét rỗng không gian ngầm trong lòng hang Deep Dark.
  - `ENCAPSULATE`: Áp dụng cho Trial Chambers để đóng hộp toàn bộ khối phòng thử thách.

---

### 4. Chiếu Trọng Lực Đường Làng (Street Projection Differential Test)

Đường làng được sinh theo chế độ `JigsawProjection::TerrainMatching`. Đường sá phải uốn lượn mềm mại theo độ cao thực tế của bề mặt địa hình tự nhiên (`HeightmapType::WorldSurfaceWg`).

Neth đã vượt qua **1,024 ca kiểm thử vi sai tự động** (`village_path_projection_1024_cases` trong `crates/pumpkin-world/src/generation/structure/template/placement_tests.rs`):
- **Phạm vi kiểm thử:** Quét qua 5 biome làng, 4 góc xoay 90/180/270 độ, và hàng trăm dạng địa hình dốc đứng, bậc thang, vách đá nhân tạo.
- **Kết quả:** **1,024 / 1,024 ca kiểm thử đều khớp chính xác 100% độ cao kỳ vọng** so với Minecraft Java Server.

---

### 5. Bộ Kiểm Thử Oracle Structure Starts Dumps

So sánh trực tiếp kết quả sinh cấu trúc của Neth với bộ dữ liệu Oracle của Minecraft Java 26.3 (`assets/tests/structure_starts.json`, kích thước 2.07 MB):
- `overworld_structure_starts_dump`: **PASSED** (Mineshaft, Village, Stronghold, Ruined Portal, Trail Ruins, Trial Chambers, Buried Treasure, Monument, Ocean Ruin...).
- `nether_structure_starts_dump`: **PASSED** (Nether Fortress, Bastion Remnant, Nether Fossil, Ruined Portal Nether...).
- `end_structure_starts_dump`: **PASSED** (End City tower/ship layout, piece positions, bounding boxes...).
