# 09. COMMANDS & DATA-DRIVEN SYSTEMS AUDIT
## ĐỐI CHIẾU HỆ THỐNG LỆNH BRIGADIER & DỮ LIỆU ĐỘNG VỚI MINECRAFT 26.3

---

### 1. Hệ thống Lệnh (Commands Subsystem)

Hệ thống lệnh của Neth là một trong những hệ thống hoàn thiện xuất sắc nhất của dự án, đạt **94.05% Implementation Coverage**:
- **79 trên tổng số 84 lệnh Vanilla** (`Commands.java`) đã được triển khai đầy đủ tệp handler trong `crates/pumpkin-core/src/command/commands/`.
- **Cây Lệnh & Phân Tích Cú Pháp (Brigadier Tree):**
  - Triển khai trong `crates/pumpkin-command`: Xây dựng cây nút (CommandNode), gợi ý tự động (Tab Completion / Suggestions), phân quyền theo cấp độ (`PermissionLevel` 0-4).
  - Xử lý mượt mà các selector thực thể phức tạp: `@a` (tất cả người chơi), `@e` (mọi thực thể), `@p` (người gần nhất), `@r` (ngẫu nhiên), `@s` (bản thân).
  - Xử lý tọa độ tương đối (`~`) và tọa độ cục bộ theo hướng nhìn (`^`).

#### Bảng Kiểm kê Các Lệnh Trọng Yếu:
| Lệnh | Vị trí Tệp trong Neth | Trạng thái Logic | Ghi chú |
| :--- | :--- | :---: | :--- |
| `/execute` | `command/commands/execute.rs` (78KB) | 🟢 Rất tốt | Hỗ trợ subcommands: `as`, `at`, `if block`, `run` |
| `/data` | `command/commands/data.rs` (40KB) | 🟢 Rất tốt | Hỗ trợ đọc/sửa NBT thực thể và khối |
| `/scoreboard` | `command/commands/scoreboard.rs` (42KB) | 🟢 Rất tốt | Objectives, Criteria, Teams, Display slots |
| `/team` | `command/commands/team.rs` (40KB) | 🟢 Rất tốt | Quản lý đội, màu sắc, prefix, collision rule |
| `/summon` | `command/commands/summon.rs` (6KB) | 🟢 Hoàn chỉnh | Spawn thực thể với NBT tùy chỉnh |
| `/give` | `command/commands/give.rs` (5KB) | 🟢 Hoàn chỉnh | Phát vật phẩm kèm Data Components |
| `/fill` / `/clone` | `command/commands/fill.rs`, `clone.rs` | 🟢 Hoàn chỉnh | Thao tác khối hàng loạt trên diện rộng |
| `/teleport` | `command/commands/teleport.rs` (15KB) | 🟢 Hoàn chỉnh | Dịch chuyển tọa độ và xoay góc nhìn |

---

### 2. Hệ thống Registries & Datapacks

- **Dynamic Registries:** Neth hỗ trợ nạp các registry động từ Datapack (Biomes, Dimensions, Damage Types, Enchantments, Trim Materials).
- **Hệ thống Tags:** Đọc và phân giải toàn bộ thẻ tag chuẩn `#minecraft:*` (Block tags, Item tags, Fluid tags, EntityType tags) phục vụ điều kiện kiểm tra gameplay.
