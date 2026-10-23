# 07. BLOCKS, ITEMS & INVENTORY SYSTEM AUDIT
## ĐỐI CHIẾU KHỐI, THỰC THỂ KHỐI, VẬT PHẨM, HÒM ĐỒ & CHẾ TẠO VỚI MINECRAFT 26.3

---

### 1. Khối & Trạng Thái Khối (Blocks & Block States)

Theo khảo sát đối chiếu trực tiếp giữa `jvmc` và Neth (chi tiết tại `docs/BLOCKS_AUDIT_REPORT.md`):

#### 1.1. Thống Kê Tổng Quan Khối
- **Tổng số khối Minecraft Vanilla 26.3:** **1,286 blocks** (tính cả các biến thể oxy hóa đồng, trạng thái gỗ, nến, v.v.).
- **Khối có hành vi động chuyên biệt (Active Rust Behaviors):** **1,031 blocks (80.17%)** được điều phối bởi **219 struct** kế thừa trait `BlockBehaviour` trong `crates/pumpkin-core/src/block/blocks/`.
- **Khối xây dựng cơ bản (Default Block Behavior):** **255 blocks (19.83%)** (đá, quặng thô, đất, cát, bê tông, kính...) sử dụng hành vi tiêu chuẩn hoàn toàn tương thích với lớp nền tảng `Block(properties)` của Minecraft Java.
- **Tỷ lệ bao phủ hiệu dụng (Effective Coverage):** **1,286 / 1,286 (100.00%)**.

#### 1.2. Phân Bổ Các Nhóm Khối Chuyên Sâu
1. **Khối Cột Xoay Trục (`RotatedPillarBlock` - 61 khối):**
   - Đã mở rộng metadata và logic xoay trục theo hướng đặt cho toàn bộ các khối log, wood, stripped logs, deepslate, basalt, bone block, purpur pillar, bamboo block trong `logs.rs`.
2. **Khối Đầu Lâu Gắn Tường (`WallSkullBlock` / `WallHeadBlock` - 7 khối):**
   - Hỗ trợ đầy đủ các biến thể đầu lâu gắn trên tường (`skeleton_wall_skull`, `player_wall_head`, `wither_wall_skull`...) tự động xoay hướng theo bề mặt và kích hoạt triệu hồi Wither khi ghép 3 đầu lâu Wither Skeleton trên cát linh hồn.
3. **Khối Tượng Đồng (`CopperGolemStatueBlock` - 8 khối):**
   - Hỗ trợ đầy đủ 8 trạng thái bình thường và sáp (`waxed`), đổi tư thế xoay vòng (standing, sitting, running, star), xuất tín hiệu Redstone Comparator (1-4) và chống ngấm nước.
4. **Khối Ngậm Nước (`Waterlogged`):**
   - Toàn bộ các khối phiến đá (slabs), bậc thang (stairs), hàng rào (fences), rương (chests) đều hỗ trợ thuộc tính `waterlogged = true/false`, tự động đồng bộ dòng chảy khi cập nhật khối láng giềng.

---

### 2. Thực Thể Khối (Block Entities - 48 / 49 Đã Triển Khai)

Trong Vanilla Minecraft 26.3 (`BlockEntityTypeIds.java`), có **49 loại Block Entity**. Neth đã triển khai struct và serialization cho **48 loại (97.96% Coverage)** tại `crates/pumpkin-core/src/block/entities/`:
- **Khối Chứa Đồ (Containers):** Rương (`ChestBlockEntity`), Rương đôi (`DoubleChest`), Thùng gỗ (`BarrelBlockEntity`), Hộp Shulker (`ShulkerBoxBlockEntity`), Phễu (`HopperBlockEntity`), Máy thả (`DropperBlockEntity`), Máy phân phát (`DispenserBlockEntity`).
- **Khối Chế Biến & Nấu Nướng:** Lò nung (`FurnaceBlockEntity`), Lò luyện kim (`BlastFurnace`), Lò hun khói (`SmokerBlockEntity`), Lửa trại (`CampfireBlockEntity`), Giàn pha thuốc (`BrewingStandBlockEntity`).
- **Khối Kỹ Thuật & Tín Hiệu:** Khối lệnh (`CommandBlockEntity`), Biển báo (`SignBlockEntity`), Đèn hải đăng (`BeaconBlockEntity`), Khối cấu trúc (`StructureBlockEntity`), Khối Jigsaw (`JigsawBlockEntity`), Trái tim Creaking (`CreakingHeartBlockEntity`).
- **Khối Trang Trí:** Khung cờ (`BannerBlockEntity`), Đầu lâu (`SkullBlockEntity`), Chuông (`BellBlockEntity`), Nồi trang trí (`DecoratedPotBlockEntity`).

---

### 3. Vật Phẩm & Kho Đồ (Items & Inventory System)

#### 3.1. Danh Mục Vật Phẩm (1,053 Items)
- **Tổng số vật phẩm Vanilla 26.3:** **1,053 items** (`ItemIds.java`).
- **Triển khai Hành vi Đặc thù:** **55 loại vật phẩm** có script logic click/sử dụng chuyên sâu trong `crates/pumpkin-core/src/item/items/`:
  - Thẻ tên (`NameTagItem`): Gán tên, hiển thị nametag, kích hoạt cờ `persistence_required = true`.
  - Cần câu cá (`FishingRodItem`): Thả mồi câu `FishingBobberEntity`, kết nối loot table `gameplay/fishing`.
  - Cây Chùy (`MaceItem`): Sát thương đập Smash Attack, hủy toàn bộ sát thương rơi, tạo sóng xung kích AOE knockback 3.5m.
  - Sáp tổ ong (`HoneycombItem`): Quét sáp bảo tồn block states của khối đồng.
  - Chai kinh nghiệm ném (`ThrownExperienceBottle`): Bắn đạn đạo rơi hạt XP 3-11 điểm.

#### 3.2. Cửa Sổ Hòm Đồ & Thao Tác Chuột (`crates/pumpkin-inventory`)
- Đã vượt qua toàn bộ unit test với **35 tests passed** kiểm thử các thao tác giao dịch:
  - **Chuột trái / Chuột phải:** Nhấc đồ, đặt 1 vật phẩm.
  - **Shift-Click (Quick Move):** Chuyển đồ thông minh giữa hòm đồ người chơi và container mục tiêu.
  - **Kéo Thả Phân Phối (Drag Split):** Kéo chuột chia đều hoặc rải 1 vật phẩm trên nhiều ô.
  - **Double-Click:** Thu gom toàn bộ các stack cùng loại vào con trỏ chuột.
  - **Phím Số Hotbar Swap:** Đổi nhanh vật phẩm giữa hotbar và container.
- **Bảo Vệ Giao Dịch (Transaction Validation):** Kiểm tra mã xác thực `state_id`, tự động đồng bộ lại nếu client có dấu hiệu desync hoặc dupe đồ.

---

### 4. Hệ Thống Chế Tạo & Nấu Nướng (Recipes Engine)

- **Cơ Sở Dữ Liệu Công Thức:** Tệp `crates/pumpkin-data/src/generated/recipes.rs` (813,512 bytes) lưu trữ hàng nghìn công thức chuẩn của Minecraft.
- **Thuật Toán So Khớp Công Thức (`recipe_matching.rs` - 3 tests PASS):**
  - Khớp công thức hình học (`ShapedRecipe`) theo ma trận lưới 2x2 hoặc 3x3.
  - Khớp công thức tự do (`ShapelessRecipe`) không phụ thuộc vị trí đặt nguyên liệu.
  - Hỗ trợ Tag-matching: Nhận diện `#minecraft:planks` cho tất cả các loại gỗ, `#minecraft:wool` cho tất cả các màu len.
- **Lò Chế Tạo Tự Động (`Crafter`):**
  - Hỗ trợ khóa từng ô trong lưới 3x3 (`disabled_slots`).
  - Khi nhận xung redstone, tự động kiểm tra công thức hợp lệ, đẩy vật phẩm thành phẩm ra mặt trước và kích hoạt thời gian hồi 4 ticks.
