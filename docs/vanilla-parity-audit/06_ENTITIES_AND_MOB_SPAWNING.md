# 06. ENTITIES & MOB SPAWNING SYSTEM AUDIT
## ĐỐI CHIẾU THỰC THỂ, HÀNH VI AI & HỆ THỐNG SPAWN VỚI MINECRAFT 26.3

---

### 1. Kiểm kê Toàn bộ Thực thể (161 Entity Types)

Toàn bộ **161 loại thực thể** trong Vanilla 26.3 (`EntityTypeIds.java`) đã được kiểm kê:
- **128 loại thực thể** đã được đăng ký và khởi tạo struct chuyên biệt trong `crates/pumpkin-core/src/entity/type.rs`.
- **Tỷ lệ Implementation Coverage:** **80.00%**.
- Các thực thể mới nhất của phiên bản 1.21 / 26.3 như `Breeze`, `Bogged`, `Creaking`, `Armadillo`, `WindCharge`, `OminousItemSpawner` đều đã có cấu trúc dữ liệu và logic cơ bản.

---

### 2. Hệ thống Sinh Quái Tự nhiên (`NaturalSpawner.rs`)

Hệ thống sinh quái tự nhiên của Neth nằm tại `crates/pumpkin-core/src/world/natural_spawner.rs` (50,063 bytes mã nguồn), tái tạo sát sao logic của `net.minecraft.world.level.NaturalSpawner.java`:

#### Cơ chế Local Mob Cap (Giới hạn Quái Cục bộ):
- Neth triển khai `LocalMobCapCalculator` sử dụng `DashMap` đa luồng hiệu năng cao.
- Kiểm tra phạm vi 17x17 chunk (`MAGIC_NUMBER = 289` chunk) xung quanh vị trí của từng người chơi.
- Giới hạn độc lập cho **8 nhóm sinh vật (Mob Categories)**:
  1. `MONSTER` (Quái vật hung hãn): Cap = 70
  2. `CREATURE` (Động vật hiền lành): Cap = 10
  3. `AMBIENT` (Dơi, môi trường): Cap = 15
  4. `AXOLOTLS`: Cap = 5
  5. `UNDERGROUND_WATER_CREATURE` (Mực phát sáng): Cap = 5
  6. `WATER_CREATURE` (Cá heo, mực): Cap = 5
  7. `WATER_AMBIENT` (Cá thường): Cap = 20
  8. `MISC`: Không giới hạn

#### Điều kiện Sinh Quái:
- Kiểm tra mức độ ánh sáng (Ánh sáng khối = 0 đối với Hostile Mobs theo chuẩn 1.18+ / 26.3).
- Kiểm tra khối sàn hợp lệ (`SpawnFloorPredicate`).
- Khoảng cách an toàn với người chơi: Không sinh trong bán kính 24 khối tính từ người chơi; tự động despawn tức thì nếu nằm ngoài 128 khối.

---

### 3. Hệ thống Trí Tuệ Nhân Tạo (Mob AI & Pathfinding)

- **Thuật toán Tìm đường A* (`pumpkin-core/src/entity/ai/pathfinder/`):**
  - Hỗ trợ đánh giá chi phí di chuyển (NodeEvaluator) trên mặt đất, bơi dưới nước, trèo bậc thang và tránh né dung nham/lửa.
- **Goal Selector & Priorities:**
  - Triển khai hàng đợi ưu tiên mục tiêu: Tấn công mục tiêu gần nhất (`MeleeAttackGoal`), Bỏ chạy khi hoảng loạn (`PanicGoal`), Nhìn ngó xung quanh (`LookAtEntityGoal`), Đi lang thang ngẫu nhiên (`WaterAvoidingRandomStrollGoal`).
- **Khoảng trống Cần Bổ sung (AI Gaps):**
  - AI Dân làng (Villager Brain): Hệ thống Sensor và Memory Modules còn ở mức cơ bản; chưa triển khai trọn vẹn chu kỳ hành vi theo thời gian trong ngày (`Schedule::VILLAGE_DEFAULT`).
  - Raid AI: Thiếu logic tập hợp toán cướp theo từng đợt sóng và kèn báo hiệu `RaidWave`.
