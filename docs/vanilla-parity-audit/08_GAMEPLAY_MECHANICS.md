# 08. GAMEPLAY MECHANICS AUDIT
## ĐỐI CHIẾU CƠ CHẾ ĐÁ ĐỎ, VẬT LÝ CHẤT LỎNG, CHIẾN ĐẤU & MÔI TRƯỜNG VỚI MINECRAFT 26.3

---

### 1. Hệ Thống Đá Đỏ (Redstone Engine)

Hệ thống Redstone trong Neth được triển khai tại `crates/pumpkin-core/src/block/blocks/redstone/` và chi tiết khảo sát đối chiếu tại `docs/redstone/java-rust-parity.md`:

#### 1.1. Bảng Đối Chiếu Các Thành Phần Cốt Lõi
| Thành phần Đá Đỏ | Mã nguồn Java (`jvmc`) | Mã nguồn Rust (`Neth`) | Đánh giá Parity | Ghi chú Hành vi |
| :--- | :--- | :--- | :---: | :--- |
| **Dây Đá Đỏ (Redstone Wire)** | `RedStoneWireBlock.java` | `redstone/redstone_wire.rs` (25KB) | 🟢 **Tốt** | Tính toán decay 15->0, shape up/side, cấp strong power xuống dưới |
| **Đuốc Đá Đỏ (Redstone Torch)** | `RedstoneTorchBlock.java` | `redstone/redstone_torch.rs` | 🟢 **Hoàn chỉnh** | Đảo chiều tín hiệu, kiểm tra tắt đuốc khi nhấp nháy quá 8 lần/60 ticks |
| **Piston & Piston Dính** | `PistonBaseBlock.java` | `piston/piston.rs` (22KB) | 🟡 **Khá** | Đẩy/kéo tối đa 12 khối, dính khối slime/honey; thiếu Quasi-Connectivity |
| **Bộ Lặp (Repeater)** | `RepeaterBlock.java` | `redstone/repeater.rs` | 🟢 **Hoàn chỉnh** | Trễ 1-4 redstone ticks (2-8 game ticks), cơ chế khóa diode bên sườn |
| **Bộ So Sánh (Comparator)** | `ComparatorBlock.java` | `redstone/comparator.rs` | 🟢 **Hoàn chỉnh** | Chế độ So sánh (Compare) và Trừ (Subtract), đọc dung tích hòm đồ |
| **Khối Quan Sát (Observer)** | `ObserverBlock.java` | `redstone/observer.rs` | 🟢 **Hoàn chỉnh** | Phát hiện thay đổi trạng thái khối mặt trước, phát xung 2 tick ra sau |
| **Thứ Tự Cập Nhật Láng Giềng** | `NeighborUpdater.java` | `world/neighbor_updater.rs` | 🟢 **Khớp 100%** | Thứ tự 6 hướng chuẩn Vanilla: `[West, East, Down, Up, North, South]` |

#### 1.2. Phân Tích Khoảng Cách Bán Dẫn Đá Đỏ (Quasi-Connectivity - QC Gap)
- **Hành vi Vanilla Java:**
  Trong `PistonBaseBlock.java:116-136`, `DispenserBlock.java:124`, `DropperBlock.java:38`:
  Piston và Dispenser nhận tín hiệu kích hoạt gián tiếp từ khối nằm cách 2 block phía trên hoặc chéo phía trên (`pos.above()`). Tín hiệu này không gửi neighbor update trực tiếp tới piston (BUD behavior), nhưng khi có bất kỳ khối nào kề cạnh piston bị cập nhật, piston sẽ kiểm tra lại và bung ra.
- **Hiện trạng Neth:**
  Hàm `should_extend` trong `crates/pumpkin-core/src/block/blocks/piston/piston.rs` hiện tại chỉ kiểm tra 6 hướng tiếp xúc trực tiếp (`level.get_redstone_power(pos.relative(direction))`).
- **Kế hoạch xử lý:** Đã tài liệu hóa thuật toán chuẩn tại `docs/redstone/java-rust-parity.md`, sẵn sàng bổ sung hàm kiểm tra `pos.above()` theo đúng đặc tả của Mojang.

---

### 2. Chất Lỏng, Trọng Lực & Vật Lý Va Chạm (Fluids & Physics)

#### 2.1. Lan Truyền Nước & Dung Nham (Fluid Dynamics)
- **Dòng Chảy Nước:**
  - Lan truyền tối đa 7 khối từ nguồn trên cạn (1 khối trong The Nether nếu dùng lệnh đặt).
  - Cơ chế tạo nguồn nước vô hạn: Khi một ô không khí tiếp xúc với ít nhất 2 nguồn nước kề cạnh có khối nâng đỡ phía dưới, ô đó lập tức biến thành nguồn nước mới.
- **Tương Tác Nước & Dung Nham:**
  - Nước chạm vào nguồn dung nham tĩnh: Tạo khối Hắc diện thạch (`Obsidian`).
  - Nước chạm vào dòng chảy dung nham: Tạo khối Đá cuội (`Cobblestone`).
  - Dung nham chảy đè lên nước: Tạo khối Đá nham thạch (`Stone`).

#### 2.2. Va Chạm Hình Hộp & Chuyển Động (AABB & Collision Engine)
- **Va chạm VoxelShape:**
  - Hỗ trợ mảng khối hộp phức tạp của hàng rào, cầu thang, vạc nước, chuông.
- **Lực Nổi & Rơi Tự Do:**
  - Thực thể chìm dưới nước nhận lực đẩy nổi (`buoyancy = 0.04`).
  - Gia tốc rơi tự do của người chơi $g = 0.08	ext{ blocks/tick}^2$ kèm lực cản không khí $0.98$.
  - Sát thương rơi tự do tính bằng $\max(0, 	ext{fall\_distance} - 3.0)$. Giảm 80% khi rơi trúng kiện cỏ khô (`HayBlock`), giảm 100% khi rơi vào nước sâu $\ge 1$ block hoặc khối chất nhờn (`SlimeBlock`).

---

### 3. Cơ Chế Chiến Đấu & Giảm Sát Thương (Combat & Damage)

#### 3.1. Công Thức Giảm Trừ Sát Thương Của Giáp (Armor Mitigation Formula)
Đối chiếu với `CombatRules.java` trong `jvmc`, Neth áp dụng công thức chuẩn xác 100%:
$$	ext{DamageMultiplier} = 1.0 - rac{\min(20, \max(	ext{armor} / 5, 	ext{armor} - rac{	ext{damage}}{2 + 	ext{toughness} / 4}))}{25}$$

#### 3.2. Thời Gian Hồi Đòn (Attack Cooldown) & Đòn Đánh Chí Mạng (Critical Hits)
- Khi thanh hồi vũ khí chưa đầy 100%, sát thương và lực đẩy lùi bị giảm tỉ lệ theo thời gian nạp.
- Khi người chơi rơi tự do (không trèo thang, không chìm trong nước, không có hiệu ứng mù) và vung đòn: Gây đòn đánh chí mạng (Critical Hit), tăng 150% sát thương cơ bản và phát sinh chùm hạt particle hình sao sắc bén.
