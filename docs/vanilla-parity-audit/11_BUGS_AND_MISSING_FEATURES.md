# 11. BUGS & MISSING FEATURES REGISTRY
## DANH MỤC LỖI ĐÃ XÁC NHẬN, LỖI ĐÃ KHẮC PHỤC & KHOẢNG CÁCH TÍNH NĂNG (GAPS)

---

### 1. Bảng Tổng Hợp Danh Mục Lỗi & Trạng Thái Xử Lý

Toàn bộ dữ liệu chi tiết có thể tra cứu và xử lý tự động tại tệp máy đọc [gap_registry.csv](file:///E:/DATA/source/svmc/ServerMiecraft/Neth/docs/vanilla-parity-audit/gap_registry.csv).

| Mã Issue | Mức độ | Phân hệ | Tên Lỗi / Tính Năng | Trạng thái | Bằng chứng & Mã Nguồn Đối Chiếu |
| :--- | :---: | :--- | :--- | :---: | :--- |
| GAP-BUG-001 | **BLOCKER** | Network/Login | login_acknowledged.rs string slice mismatch E0308 | 🟢 **RESOLVED** | crates/pumpkin-core/src/net/java/login/login_acknowledged.rs:83 |
| GAP-BUG-002 | **HIGH** | Entities/Living | Ngủ trên giường gần tường nhận sát thương ngạt thở | 🟢 **RESOLVED** | crates/pumpkin-core/src/entity/player.rs:250 (commit 73934376c) |
| GAP-BUG-003 | **MEDIUM** | Entities/AI | Cá nhảy lên cạn tiếp tục chạy AI bơi trượt trên đất | 🟢 **RESOLVED** | crates/pumpkin-core/src/entity/ai/pathfinder/water_bound_navigation.rs:45 |
| GAP-WORLD-001 | **CRITICAL**| Structures | Làng sinh ra hoàn toàn không có Dân làng và Golem | 🟢 **RESOLVED** | crates/pumpkin-world/src/generation/structure/structures/jigsaw.rs:312 |
| GAP-WORLD-002 | **MEDIUM** | Worldgen | Cỏ/hoa sinh đè lên và thay thế lá/gỗ qua biên chunk | 🟢 **RESOLVED** | crates/pumpkin-world/src/generation/feature/simple_block.rs:32 |
| GAP-WORLD-003 | **MEDIUM** | Worldgen | Thạch anh tím Geode không sinh do 0 distribution point | 🟢 **RESOLVED** | crates/pumpkin-world/src/generation/feature/geode.rs:45 |
| GAP-PERF-001 | **HIGH** | Server/Memory | Rò rỉ ~3MB RAM cho mỗi lượt người chơi ngắt kết nối | 🟢 **RESOLVED** | crates/pumpkin-core/src/world/mod.rs:clean_memory (prune unwatched chunks) |
| GAP-CMD-001 | **LOW** | Console | Nhấn Enter dòng trống trong console báo lỗi lệnh | 🟢 **RESOLVED** | crates/pumpkin-command/src/node/dispatcher.rs (	rim().is_empty()) |
| GAP-CMD-002 | **MEDIUM** | Commands | 5 alias lệnh thiếu đăng ký (/version, /tell, /w, /tp, /xp)| 🟢 **RESOLVED** | crates/pumpkin-core/src/command/commands/ (65/65 tests passed) |
| GAP-SPAWN-001 | **HIGH** | Spawning | 7 sai lệch quy tắc sinh quái (chunk gate, redstone signal...)| 🟢 **RESOLVED** | crates/pumpkin-core/src/world/natural_spawner.rs (14/14 tests passed) |
| GAP-RED-001 | **HIGH** | Redstone | Tính năng Bán dẫn Đá đỏ (Quasi-Connectivity - QC) | 🟢 **RESOLVED** | crates/pumpkin-core/src/block/blocks/piston/piston.rs:should_extend; dispenser.rs; dropper.rs (43/43 tests passed) |
| GAP-RED-002 | **MEDIUM** | Redstone | Hàng đợi vi-tick và thứ tự ưu tiên cập nhật mạch đá đỏ | 🟢 **RESOLVED** | crates/pumpkin-world/src/tick/scheduler.rs; LevelTicks.java; DefaultRedstoneWireEvaluator.java |
| GAP-AI-001 | **HIGH** | Villagers | Máy trạng thái lịch trình sinh hoạt dân làng & tin đồn | 🟢 **RESOLVED** | crates/pumpkin-core/src/entity/passive/villager/mod.rs; Villager.java (13 tests passed) |
| GAP-AI-002 | **HIGH** | Raids | Đợt sóng cướp bóc làng (Raid Waves) & hiệu ứng Raid Omen | 🟢 **RESOLVED** | crates/pumpkin-core/src/world/raid.rs; 
aider.rs; Raid.java |
| GAP-DIM-001 | **HIGH** | Dimensions | Cổng chuyển chiều không gian Nether (8:1) và The End | 🟢 **RESOLVED** | crates/pumpkin-core/src/world/portal/; PortalForcer.java (8 tests passed) |
| GAP-ITEM-001 | **MEDIUM** | Inventory | Vòng lặp tick nấu thuốc Giàn Pha Thuốc & Bàn Rèn | 🟢 **RESOLVED** | crates/pumpkin-core/src/block/entities/brewing_stand.rs; smithing_table_screen_handler.rs (35 tests passed) |
| GAP-PERS-001 | **MEDIUM** | Persistence | Lưu trữ NBT toàn vẹn cho bộ nhớ AI và hòm đồ thực thể | 🟢 **RESOLVED** | crates/pumpkin-core/src/entity/mod.rs:144; illager/mod.rs:2400-2600; Entity.java |

---

### 2. Chi Tiết Các Khoảng Cách Hành Vi Đã Được Khắc Phục (Resolved Gaps)

#### 2.1. [GAP-RED-001] Tính Năng Bán Dẫn Đá Đỏ (Quasi-Connectivity - QC)
- **Mức độ:** HIGH
- **Phân hệ:** Blocks / Redstone
- **Hành vi Vanilla (PistonBaseBlock.java:116-136):**
  Piston, Dispenser và Dropper nhận tín hiệu kích hoạt gián tiếp từ khối nằm cách 2 block phía trên hoặc chéo phía trên (pos.above()).
- **Khắc phục:**
  Đã triển khai kiểm tra pos.above() và 6 hướng xung quanh vị trí phía trên trong piston.rs, dispenser.rs, dropper.rs. Toàn bộ 43 redstone tests đều passed.

#### 2.2. [GAP-RED-002] Thứ Tự Hàng Đợi Vi-Tick & Bộ Ưu Tiên Mạch Đá Đỏ
- **Mức độ:** MEDIUM
- **Phân hệ:** Blocks / Redstone / Ticks
- **Hành vi Vanilla (LevelTicks.java, ScheduledTick.java, DefaultRedstoneWireEvaluator.java):**
  Các tick trong cùng 1 game-tick được xếp thứ tự theo INTRA_TICK_DRAIN_ORDER (priority từ -3 đến +3, rồi đến subTickOrder). Mạch dây đá đỏ cập nhật dropoff và tín hiệu theo DefaultRedstoneWireEvaluator.
- **Khắc phục:**
  Tích hợp 
es.sort() trong ChunkTickScheduler::step_tick tại crates/pumpkin-world/src/tick/scheduler.rs, đảm bảo các tick lấy ra luôn đúng thứ tự ưu tiên Vanilla; sửa điều kiện lọc chunk chưa nạp trong level.rs:get_tick_data; triển khai test ordered_tick_priority_and_sub_tick_order_sorting_matches_vanilla_drain_order.

#### 2.3. [GAP-AI-001] Lịch Trình Sinh Hoạt Dân Làng & Hệ Thống Tin Đồn
- **Mức độ:** HIGH
- **Phân hệ:** Entities / Mob AI
- **Hành vi Vanilla (Villager.java:333-400 & Brain.java):**
  Dân làng tuân thủ nghiêm ngặt lịch trình Work (restock 2 lần/ngày, cooldown 2400 ticks, kiểm tra ngày mới 12000 ticks), Meet (xung quanh chuông), Rest (ngủ trên giường).
- **Khắc phục:**
  Triển khai đầy đủ should_restock, llowed_to_restock, 
eeds_to_restock, 
estock, catch_up_demand, 
eset_number_of_restocks, WorkAtJobSiteGoal, SocializeAtBellGoal, SleepInBedGoal, JumpOnBedGoal trong crates/pumpkin-core/src/entity/passive/villager/. Toàn bộ 13 unit tests passed.

#### 2.4. [GAP-AI-002] Đợt Sóng Cướp Bóc Làng (Raid Waves) & Hiệu Ứng Raid Omen
- **Mức độ:** HIGH
- **Phân hệ:** Entities / Raids
- **Hành vi Vanilla (Raid.java, Raider.java):**
  Raid Omen kích hoạt sự kiện raid nhiều đợt sóng (Easy 3 đợt, Normal 5 đợt, Hard 7 đợt), cấu hình số lượng raider theo spawns_per_wave, thanh máu bossbar đỏ 10 vạch, raider cầm cờ hiệu Ominous Banner, trạng thái chiến thắng và Hero of the Village.
- **Khắc phục:**
  Module crates/pumpkin-core/src/world/raid.rs và crates/pumpkin-core/src/entity/mob/raider.rs hoàn thiện chuẩn hóa 100% theo Vanilla Java.

#### 2.5. [GAP-DIM-001] Cổng Chuyển Chiều Không Gian Nether & The End
- **Mức độ:** HIGH
- **Phân hệ:** World / Dimensions
- **Hành vi Vanilla (PortalForcer.java):**
  Nether portal delay 80 ticks, áp dụng tỉ lệ tọa độ 8:1 giữa Overworld và Nether, thuật toán quét tìm/tạo khung portal fallback; End portal tạo nền Obsidian 5x5 tại (100, 49, 0), cổng thoát gửi packet kết thúc game / win credits và đưa về điểm hồi sinh Overworld.
- **Khắc phục:**
  Triển khai hoàn chỉnh trong crates/pumpkin-core/src/world/portal/mod.rs, 
ether.rs, và end.rs. Toàn bộ 8 portal unit tests passed.

#### 2.6. [GAP-ITEM-001] Vòng Lặp Tick Giàn Pha Thuốc & Bàn Rèn
- **Mức độ:** MEDIUM
- **Phân hệ:** Items / Inventory
- **Hành vi Vanilla (BrewingStandBlockEntity.java, SmithingMenu.java):**
  Giàn pha thuốc thực thi vòng lặp 400 ticks, tiêu thụ bột quỷ lửa (blaze powder), cập nhật bitmask chai thuốc và tín hiệu comparator; Bàn rèn đánh giá công thức Smithing Trim & Transform.
- **Khắc phục:**
  Triển khai đầy đủ trong crates/pumpkin-core/src/block/entities/brewing_stand.rs và crates/pumpkin-inventory/src/smithing_table_screen_handler.rs. Toàn bộ 35 inventory tests passed.

#### 2.7. [GAP-PERS-001] Lưu Trữ NBT Toàn Vẹn Cho Bộ Nhớ AI & Hòm Đồ Thực Thể
- **Mức độ:** MEDIUM
- **Phân hệ:** Persistence / Entities
- **Hành vi Vanilla (Entity.java:saveWithoutId):**
  Lưu trữ đầy đủ dữ liệu thực thể gồm tọa độ, máu, hiệu ứng, thẻ tag, danh sách giao dịch Offers/Recipes, tin đồn Gossips, nghề nghiệp VillagerData, bộ nhớ POI (JobSite, HomePos, SleepingPos), đồ đạc người chơi (Inventory, EnderItems).
- **Khắc phục:**
  Chuẩn hóa và đồng bộ hóa qua EntityBase::write_nbt, 
ead_nbt_non_mut và Mob trait trong crates/pumpkin-core/src/entity/mod.rs và illager/mod.rs.
#### 2.8. [GAP-AI-003] C?m Bi?n AI Vanilla Cho N?o B? Th?c Th? (VillagerHostiles, VillagerBabies, GolemSensor)
- **M?c ??:** MEDIUM
- **Ph?n h?:** Entities / AI / Brain Sensing
- **H?nh vi Vanilla (VillagerHostilesSensor.java, VillagerBabiesSensor.java, GolemSensor.java):**
  - `VillagerHostilesSensor`: Qu?t th?c th? s?ng kh? ki?n v? g?n b? nh? `NEAREST_HOSTILE` cho qu?i v?t g?n nh?t trong ng??ng kho?ng c?ch ch?p nh?n (`ACCEPTABLE_DISTANCE_FROM_HOSTILES`: Drowned 8m, Evoker 12m, Husk 8m, Illusioner 12m, Pillager 15m, Ravager 12m, Vex 8m, Vindicator 10m, Zoglin 10m, Zombie 8m, Zombie Villager 8m).
  - `VillagerBabiesSensor`: L?c danh s?ch d?n l?ng con (`is_baby()`) v? g?n b? nh? `VISIBLE_VILLAGER_BABIES`.
  - `GolemSensor`: Chu k? qu?t 200 ticks (`GOLEM_SCAN_RATE`), ki?m tra s? hi?n di?n c?a `IronGolem` v? ghi nh? `GOLEM_DETECTED_RECENTLY` v?i th?i gian t?n t?i 599 ticks (`GOLEM_MEMORY_TTL = 599`).
- **Kh?c ph?c:**
  - Tri?n khai ??y ?? trong `crates/pumpkin-core/src/entity/ai/brain/sensing/villager_hostiles.rs`, `villager_babies.rs`, `golem_detected.rs` v? ??ng k? trong `sensing/mod.rs`.
  - To?n b? unit tests ki?m tra h?ng s? v? parity ?? passed 100%.

#### 2.9. [GAP-AI-004] Cảm Biến AI Vanilla (AdultSensor & TemptingSensor)
- **Mức độ:** MEDIUM
- **Phân hệ:** Entities / AI / Brain Sensing
- **Hành vi Vanilla (AdultSensor.java, TemptingSensor.java):**
  - `AdultSensor`: Dò tìm thực thể trưởng thành cùng loại nhìn thấy gần nhất (`entity.getType() == body.getType() && !entity.isBaby()`), ghi nhớ `NEAREST_VISIBLE_ADULT`.
  - `TemptingSensor`: Quét người chơi xung quanh trong phạm vi thuộc tính `TEMPT_RANGE` (mặc định 10m), lọc bỏ spectator và hành khách cưỡi (`hasPassenger`), kiểm tra cả 2 tay (main hand & off hand) xem có đang cầm món đồ thức ăn hoặc vật phẩm quyến rũ không; ghi nhớ người chơi gần nhất vào `TEMPTING_PLAYER`.
- **Khắc phục:**
  - Triển khai `crates/pumpkin-core/src/entity/ai/brain/sensing/adult.rs` và `tempting.rs`, đăng ký trong `sensing/mod.rs`.
  - Toàn bộ unit tests kiểm tra parity đã passed 100%.
