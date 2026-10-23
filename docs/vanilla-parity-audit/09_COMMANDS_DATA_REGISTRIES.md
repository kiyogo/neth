# 09. COMMANDS, REGISTRIES & DATA-DRIVEN SYSTEMS AUDIT
## ĐỐI CHIẾU HỆ THỐNG LỆNH BRIGADIER, DỮ LIỆU ĐỘNG & REGISTRY TAGS VỚI MINECRAFT 26.3

---

### 1. Hệ Thống Lệnh (Brigadier Command Subsystem - 100% Coverage)

Theo kết quả kiểm toán và đối chiếu chi tiết trong `docs/COMMANDS_AUDIT_REPORT.md`:
- **Tổng số lớp lệnh trong Vanilla Java (`jvmc`):** 97 lớp lệnh.
- **Tổng số lệnh Standard + Dedicated Vanilla:** **87 lệnh**.
- **Tổng số Root Literals & Aliases:** **102 literals** (bao gồm `/tell`, `/w`, `/tp`, `/xp`, `/setspawn`, `/ban-ip`, v.v.).
- **Đã đăng ký và kích hoạt trong Neth:** **102 / 102 literals (100.00%)**.
- **Kiểm thử tự động:** Vượt qua **91/91 tests** trong `pumpkin-command` và **65/65 tests** trong `pumpkin-core` (tổng 156 tests lệnh passed 100%).

#### 1.1. Bảng Tra Cứu Toàn Bộ Các Lệnh Vanilla Trọng Yếu
| Lệnh | Vị trí Tệp trong Neth | Độ Phức Tạp | Đánh giá Parity | Ghi chú Logic |
| :--- | :--- | :---: | :---: | :--- |
| `/execute` | `command/commands/execute.rs` (78KB) | Rất cao | 🟢 **10/10 tests PASS** | `as`, `at`, `in`, `positioned`, `if/unless block/entity/score`, `store` |
| `/scoreboard` | `command/commands/scoreboard.rs` (42KB) | Cao | 🟢 **9/9 tests PASS** | Objectives, 9 toán tử operations, sidebar, belowName, list |
| `/team` | `command/commands/team.rs` (40KB) | Cao | 🟢 **3/3 tests PASS** | Add/remove/join/leave/empty, prefix/suffix, deathMessageVisibility |
| `/version` | `command/commands/version.rs` | Trung bình | 🟢 **100% Khớp jvmc** | Tái hiện `VersionCommand.java` với Header, DataVersion 5023, Protocol 777 |
| `/teleport` (`/tp`)| `command/commands/teleport.rs` (15KB) | Cao | 🟢 **Hoàn chỉnh** | Tọa độ tuyệt đối, tương đối `~`, góc nhìn `^`, xoay facing |
| `/summon` | `command/commands/summon.rs` (6KB) | Trung bình | 🟢 **Hoàn chỉnh** | Triệu hồi thực thể kèm SNBT compound tùy biến |
| `/give` | `command/commands/give.rs` (5KB) | Trung bình | 🟢 **Hoàn chỉnh** | Cấp phát vật phẩm kèm Data Components |
| `/fill` / `/clone` | `command/commands/fill.rs`, `clone.rs` | Cao | 🟢 **Hoàn chỉnh** | Thao tác khối hàng loạt theo thể tích AABB |
| `/data` | `command/commands/data.rs` (40KB) | Rất cao | 🟢 **Hoàn chỉnh** | Đọc, gộp, sửa đổi NBT Path của Block, Entity và Storage |
| `/gamerule` | `command/commands/gamerule.rs` | Trung bình | 🟢 **Hoàn chỉnh** | Toàn bộ các gamerule chuẩn (keepInventory, doDaylightCycle...) |

#### 1.2. Phân Tích Chuyên Sâu 3 Lệnh Phức Tạp Nhất
1. **Lệnh `/execute`:**
   - Hỗ trợ đầy đủ các modifier thực thể: `as`, `at`, `on <relation>` (`owner`, `leasher`, `target`, `attacker`, `vehicle`, `passengers`).
   - Hỗ trợ các modifier tọa độ: `positioned`, `positioned over <heightmap>`, `rotated`, `align`, `anchored`.
   - Hỗ trợ điều kiện lọc logic: `if block`, `if blocks <all|masked>`, `if entity`, `if score <op>`.
   - Hỗ trợ lưu trữ kết quả: `store result` và `store success` vào score, block NBT, entity NBT, bossbar.
2. **Lệnh `/scoreboard`:**
   - Quản lý mục tiêu (`objectives`): hỗ trợ renderType, numberformat, hiển thị tên tùy chỉnh.
   - Hỗ trợ đầy đủ 9 toán tử số học: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `><` (hoán đổi), `<` (lấy min), `>` (lấy max).
3. **Lệnh `/team`:**
   - Hỗ trợ đầy đủ các tùy chọn: `displayName`, `color`, `friendlyFire`, `seeFriendlyInvisibles`, `nametagVisibility`, `deathMessageVisibility`, `collisionRule`, `prefix`, `suffix`.

---

### 2. Hệ Thống Registries & Datapacks Động

#### 2.1. Dynamic Registries Bootstrapping
Neth nạp các registry động từ thư mục dữ liệu chuẩn của Minecraft:
- `minecraft:worldgen/biome`: Đầy đủ 67 biomes Vanilla.
- `minecraft:dimension_type`: Overworld, The Nether, The End.
- `minecraft:damage_type`: Sát thương cận chiến, tầm xa, nổ, cháy, xuyên giáp.
- `minecraft:enchantment`: Đầy đủ bảng bùa chú phiên bản 1.21.
- `minecraft:trim_pattern` & `minecraft:trim_material`: Hoa văn và vật liệu ép giáp.

#### 2.2. Hệ Thống Thẻ Tag (`#minecraft:*`)
Crate `pumpkin-data` tải và giải mã toàn bộ cây thẻ tag chuẩn từ Vanilla Data Pack:
- **Block Tags:** `#minecraft:mineable/pickaxe`, `#minecraft:dirt`, `#minecraft:logs`, `#minecraft:doors`, `#minecraft:geode_invalid_blocks`.
- **Item Tags:** `#minecraft:swords`, `#minecraft:pickaxes`, `#minecraft:arrows`, `#minecraft:coals`.
- **Fluid Tags:** `#minecraft:water`, `#minecraft:lava`.
- **Entity Tags:** `#minecraft:undead`, `#minecraft:skeletons`, `#minecraft:raiders`.

---

### 3. Bảng Rơi Đồ (Loot Tables) & Tiến Trình (Advancements)

- **Loot Tables:** Triển khai trong `crates/pumpkin-data` và `crates/pumpkin-core/src/loot/`:
  - Bảng loot khối: Rơi block khi khai thác bằng công cụ tương ứng (kèm bùa Fortune và Silk Touch).
  - Bảng loot câu cá: `gameplay/fishing` hỗ trợ 4 phân loại đồ: Cá (`Fish`), Rác (`Junk`), Kho báu (`Treasure`).
- **Tiến Trình (Advancements):**
  - Đã gắn trigger cho các hành động gameplay: `slept_in_bed`, `bred_animals`, `fished_item`.
  - Ghi nhận thống kê cá nhân (`PlayerStats`): số lần chết, số mob tiêu diệt, khoảng cách di chuyển.
