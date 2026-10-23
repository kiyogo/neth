# 11. BUGS & MISSING FEATURES REGISTRY
## DANH MỤC LỖI ĐÃ XÁC NHẬN, LỖI ĐÃ KHẮC PHỤC & KHOẢNG CÁCH TÍNH NĂNG (GAPS)

---

### 1. Bảng Tổng Hợp Danh Mục Lỗi & Trạng Thái Xử Lý

Toàn bộ dữ liệu chi tiết có thể tra cứu và xử lý tự động tại tệp máy đọc [gap_registry.csv](file:///E:/DATA/source/svmc/ServerMiecraft/Neth/docs/vanilla-parity-audit/gap_registry.csv).

| Mã Issue | Mức độ | Phân hệ | Tên Lỗi / Tính Năng | Trạng thái | Bằng chứng & Mã Nguồn Đối Chiếu |
| :--- | :---: | :--- | :--- | :---: | :--- |
| `GAP-BUG-001` | **BLOCKER** | Network/Login | `login_acknowledged.rs` string slice mismatch E0308 | 🟢 **RESOLVED** | `crates/pumpkin-core/src/net/java/login/login_acknowledged.rs:83` |
| `GAP-BUG-002` | **HIGH** | Entities/Living | Ngủ trên giường gần tường nhận sát thương ngạt thở | 🟢 **RESOLVED** | `crates/pumpkin-core/src/entity/player.rs:250` (commit `73934376c`) |
| `GAP-BUG-003` | **MEDIUM** | Entities/AI | Cá nhảy lên cạn tiếp tục chạy AI bơi trượt trên đất | 🟢 **RESOLVED** | `crates/pumpkin-core/src/entity/ai/pathfinder/water_bound_navigation.rs:45` |
| `GAP-WORLD-001` | **CRITICAL**| Structures | Làng sinh ra hoàn toàn không có Dân làng và Golem | 🟢 **RESOLVED** | `crates/pumpkin-world/src/generation/structure/structures/jigsaw.rs:312` |
| `GAP-WORLD-002` | **MEDIUM** | Worldgen | Cỏ/hoa sinh đè lên và thay thế lá/gỗ qua biên chunk | 🟢 **RESOLVED** | `crates/pumpkin-world/src/generation/feature/simple_block.rs:32` |
| `GAP-WORLD-003` | **MEDIUM** | Worldgen | Thạch anh tím Geode không sinh do 0 distribution point | 🟢 **RESOLVED** | `crates/pumpkin-world/src/generation/feature/geode.rs:45` |
| `GAP-PERF-001` | **HIGH** | Server/Memory | Rò rỉ ~3MB RAM cho mỗi lượt người chơi ngắt kết nối | 🟢 **RESOLVED** | `crates/pumpkin-core/src/world/mod.rs:clean_memory` (prune unwatched chunks) |
| `GAP-CMD-001` | **LOW** | Console | Nhấn Enter dòng trống trong console báo lỗi lệnh | 🟢 **RESOLVED** | `crates/pumpkin-command/src/node/dispatcher.rs` (`trim().is_empty()`) |
| `GAP-CMD-002` | **MEDIUM** | Commands | 5 alias lệnh thiếu đăng ký (`/version`, `/tell`, `/w`, `/tp`, `/xp`)| 🟢 **RESOLVED** | `crates/pumpkin-core/src/command/commands/` (65/65 tests passed) |
| `GAP-SPAWN-001` | **HIGH** | Spawning | 7 sai lệch quy tắc sinh quái (chunk gate, redstone signal...)| 🟢 **RESOLVED** | `crates/pumpkin-core/src/world/natural_spawner.rs` (14/14 tests passed) |
| `GAP-RED-001` | **HIGH** | Redstone | Thiếu tính năng Bán dẫn Đá đỏ (Quasi-Connectivity) | 🔴 **ACTIVE_GAP**| `crates/pumpkin-core/src/block/blocks/piston/piston.rs:should_extend` |
| `GAP-RED-002` | **MEDIUM** | Redstone | Hàng đợi vi-tick và thứ tự ưu tiên cập nhật mạch đá đỏ | 🟡 **ACTIVE_GAP**| `crates/pumpkin-world/src/tick.rs`; `DefaultRedstoneWireEvaluator.java` |
| `GAP-AI-001` | **HIGH** | Villagers | Máy trạng thái lịch trình sinh hoạt dân làng & tin đồn | 🔴 **ACTIVE_GAP**| `crates/pumpkin-core/src/entity/mob/villager/`; `Brain.java` trong `jvmc` |
| `GAP-AI-002` | **HIGH** | Raids | Đợt sóng cướp bóc làng (Raid Waves) & hiệu ứng Bad Omen | 🔴 **ACTIVE_GAP**| `crates/pumpkin-core/src/entity/raid/`; `Raid.java` trong `jvmc` |
| `GAP-DIM-001` | **HIGH** | Dimensions | Cổng chuyển chiều không gian Nether (8:1) và The End | 🔴 **ACTIVE_GAP**| `crates/pumpkin-core/src/world/portal/nether.rs`; `PortalForcer.java` |
| `GAP-ITEM-001` | **MEDIUM** | Inventory | Vòng lặp tick nấu thuốc Giàn Pha Thuốc & Bàn Rèn | 🟡 **ACTIVE_GAP**| `crates/pumpkin-core/src/block/entities/brewing_stand.rs` |
| `GAP-PERS-001` | **MEDIUM** | Persistence | Lưu trữ NBT toàn vẹn cho bộ nhớ AI và hòm đồ thực thể | 🟡 **ACTIVE_GAP**| `crates/pumpkin-world/src/chunk/format/anvil.rs`; `Entity.java:saveWithoutId` |

---

### 2. Chi Tiết Các Khoảng Cách Hành Vi Đang Được Xử Lý (Active Gaps)

#### 2.1. [GAP-RED-001] Thiếu Tính Năng Bán Dẫn Đá Đỏ (Quasi-Connectivity - QC)
- **Mức độ nghiêm trọng:** `HIGH`
- **Phân hệ ảnh hưởng:** `Blocks / Redstone`
- **Hành vi Vanilla (`PistonBaseBlock.java:116-136`):**
  Piston, Dispenser và Dropper nhận tín hiệu kích hoạt gián tiếp từ khối nằm cách 2 block phía trên hoặc chéo phía trên (`pos.above()`). Khi khối kề cạnh có cập nhật, piston kiểm tra lại nguồn cấp và kích hoạt đẩy.
- **Hành vi Neth hiện tại:**
  Chỉ kiểm tra tín hiệu trực tiếp từ 6 mặt kề sát (`pos.relative(direction)`).
- **Phương án khắc phục:**
  Bổ sung hàm kiểm tra `pos.above()` và 6 hướng xung quanh vị trí phía trên theo đúng đặc tả của Mojang.

#### 2.2. [GAP-AI-001] Lịch Trình Sinh Hoạt Dân Làng & Hệ Thống Tin Đồn (Villager Brain & Gossip)
- **Mức độ nghiêm trọng:** `HIGH`
- **Phân hệ ảnh hưởng:** `Entities / Mob AI`
- **Hành vi Vanilla (`Villager.java` & `Brain.java`):**
  Dân làng tuân thủ nghiêm ngặt thời gian biểu trong ngày:
  1. `WORK` (7:00 - 15:00): Làm việc tại khối nghề nghiệp POI, bổ sung lượng hàng hóa (Restocking 2 lần/ngày).
  2. `MEET` (15:00 - 18:00): Tụ tập quanh chuông làng, trao đổi tin đồn (`GossipType::MajorPositive`, `MinorNegative`...), giảm giá cho người chơi giải cứu làng.
  3. `REST` (18:00 - 6:00 sáng hôm sau): Tìm giường nằm ngủ, đặt lại điểm hồi sinh.
- **Hành vi Neth hiện tại:**
  Dân làng mới chỉ nhận nghề cơ bản qua POI workstation và đi lang thang; chưa có máy trạng thái chuyển đổi hoạt động theo giờ.
- **Phương án khắc phục:**
  Triển khai máy trạng thái lịch trình `VillagerSchedule` và module lưu trữ `Gossip` trong `crates/pumpkin-core`.

#### 2.3. [GAP-DIM-001] Cổng Chuyển Chiều Không Gian Nether & The End
- **Mức độ nghiêm trọng:** `HIGH`
- **Phân hệ ảnh hưởng:** `World / Dimensions`
- **Hành vi Vanilla (`PortalForcer.java`):**
  Khi người chơi đứng trong cổng Nether quá thời gian trễ (80 ticks):
  1. Tính toán tọa độ đích: $X_{\text{dest}} = X / 8$, $Z_{\text{dest}} = Z / 8$.
  2. Quét tìm cổng Nether có sẵn trong bán kính 128 khối ở chiều đích.
  3. Nếu không có cổng, tự động đặt khung cổng Obsidian kèm khối Nether Portal và hai ngọn đuốc.
- **Hành vi Neth hiện tại:**
  Khung cổng và va chạm đã nhận diện, nhưng luồng dịch chuyển thực thể giữa các chiều không gian kèm thuật toán tìm/tạo cổng đang để lại stub.
- **Phương án khắc phục:**
  Triển khai pipeline dịch chuyển chiều không gian `DimensionTravelPipeline` trong `crates/pumpkin-core`.
