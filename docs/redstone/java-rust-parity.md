# Tài liệu Đối chiếu Kỹ thuật Redstone Parity (Java vs Rust)

Tài liệu này lưu trữ bảng đối chiếu chi tiết giữa mã nguồn tham chiếu Minecraft Java Edition (`jvmc` - Minecraft 1.21.9 / Protocol 777) và engine máy chủ Neth viết bằng Rust (`crates/pumpkin-core`), phục vụ việc chuẩn hóa 100% cơ chế Redstone.

## 1. Bảng đối chiếu các thành phần cốt lõi

| Thành phần | Mã nguồn Java (`jvmc`) | Mã nguồn Rust (`Neth`) | Hành vi hiện tại | Sai khác đã xác minh | Kế hoạch xử lý |
|---|---|---|---|---|---|
| **Redstone Power (Weak vs Strong)** | `SignalGetter.java:13-106`<br>`BlockBehaviour.java:368-382` | `crates/pumpkin-core/src/block/blocks/redstone/mod.rs`<br>`get_redstone_power`<br>`get_max_strong_power` | Đã phân tách hàm tính công suất weak/strong qua block registry và solid block. | `RedstoneWireBlock::get_strong_redstone_power` trả về `wire.power` cho cả các hướng ngang (horizontal) nếu có kết nối, khiến khối đặc kề bên bị cấp Strong Power sai quy tắc vanilla. | Sửa `redstone_wire.rs` chỉ trả về `wire.power` cho `BlockDirection::Up` (cấp strong power xuống dưới), trả về 0 cho tất cả hướng ngang. |
| **Redstone Wire (Shape, Power, Updates)** | `RedStoneWireBlock.java:43-517`<br>`DefaultRedstoneWireEvaluator.java:12-44` | `crates/pumpkin-core/src/block/blocks/redstone/redstone_wire.rs` | Đã có tính toán decay (-1), tính toán shape (none/side/up), lan truyền tín hiệu mạng dây. | Wire phát strong power theo chiều ngang. Khi dây bị phá/thay đổi trạng thái, cần đảm bảo notify đầy đủ khối lân cận để tránh tín hiệu dư thừa hoặc lặp vô hạn. | Sửa `get_strong_redstone_power`; kiểm tra và bảo đảm update neighbors đồng bộ theo vanilla. |
| **Neighbor Update & Order** | `NeighborUpdater.java:24-34`<br>`[West, East, Down, Up, North, South]` | `crates/pumpkin-core/src/world/neighbor_updater.rs` | Đã dùng hằng số thứ tự cập nhật tương đương Java `[West, East, Down, Up, North, South]`. | Đã khớp thứ tự với vanilla Java `NeighborUpdater.UPDATE_ORDER`. Cần kiểm tra flag và chặn đệ quy vô hạn. | Giữ nguyên thứ tự cập nhật chuẩn, tăng cường test kiểm chứng thứ tự cập nhật 6 hướng. |
| **Scheduled Tick & Priority** | `Level.java`<br>`LevelTicks.java` | `crates/pumpkin-world/src/tick.rs`<br>`World::schedule_block_tick` | Hệ thống scheduled tick hỗ trợ các mức độ ưu tiên `TickPriority` (`ExtremelyHigh`, `VeryHigh`, `High`, `Normal`). | Các cổng diode (repeater, comparator) và dispenser/dropper sử dụng tick priority tương ứng vanilla. Cần kiểm tra độ trễ 2 redstone tick (4 game tick) cho repeater 1-delay, 4 tick cho dispenser. | Đảm bảo scheduled tick priority và delay được duy trì chính xác trong `repeater.rs`, `comparator.rs`, `dispenser.rs`, `dropper.rs`. |
| **Piston & Sticky Piston** | `PistonBaseBlock.java:30-384`<br>`PistonStructureResolver.java`<br>`PistonMovingBlockEntity.java` | `crates/pumpkin-core/src/block/blocks/piston/piston.rs`<br>`piston_handler.rs`<br>`entities/piston.rs` | Đẩy/kéo tối đa 12 block, hỗ trợ block không thể đẩy (obsidian, v.v.), cơ chế dính slime/honey. | Trong `finish()` của `PistonBlockEntity`, khi kết thúc animation rút piston hoặc ngắt đột ngột, blockstate cần đặt đúng tương tự vanilla `finalTick()`. | Đối chiếu và chuẩn hóa `finish()` và `try_move()`, bảo đảm trạng thái block sau khi hoàn thành animation khớp vanilla. |
| **Quasi-Connectivity (QC)** | `PistonBaseBlock.java:116-136`<br>`DispenserBlock.java:124`<br>`DropperBlock.java:38` | `crates/pumpkin-core/src/block/blocks/piston/piston.rs`<br>`dispenser.rs`<br>`dropper.rs` | `should_extend` trong `piston.rs` duyệt `pos.up()`, dispenser/dropper kiểm tra `pos.up()`. | 1) Piston `should_extend` cần kiểm tra chuẩn xác `has_signal` cho cả 6 hướng xung quanh vị trí `pos.above()` (trừ `Down`), và tín hiệu trực tiếp; 2) BUD behavior: nguồn QC gián tiếp không gửi neighbor update trực tiếp tới piston/dispenser, chỉ khi có block kề bên cập nhật mới kích hoạt. | Chuẩn hóa `should_extend` trong `piston.rs` khớp với `PistonBaseBlock.getNeighborSignal`; chuẩn hóa `on_neighbor_update` của dispenser/dropper; bổ sung test case BUD/QC. |
| **Repeater (Diode)** | `RepeaterBlock.java:23-117`<br>`DiodeBlock.java:20-204` | `crates/pumpkin-core/src/block/blocks/redstone/repeater.rs`<br>`abstract_redstone_gate.rs` | Hỗ trợ 4 mức delay (1-4 redstone ticks), kiểm tra khóa (locking) từ diode bên cạnh. | Cập nhật thuộc tính `locked` cần đồng bộ cả trong `on_neighbor_update` lẫn `get_state_for_neighbor_update` (shape update) khi diode bên sườn thay đổi trạng thái kích hoạt. | Thêm đồng bộ trạng thái `locked` khi nhận `on_neighbor_update`, bảo đảm trạng thái hiển thị và hành vi khóa luôn nhất quán. |
| **Comparator (Diode & Container)** | `ComparatorBlock.java:25-180`<br>`DiodeBlock.java:20-204` | `crates/pumpkin-core/src/block/blocks/redstone/comparator.rs`<br>`abstract_redstone_gate.rs` | Hỗ trợ chế độ Compare và Subtract, đọc tín hiệu từ container và khối phía sau. | Cần kiểm tra parity việc đọc container xuyên qua 1 khối solid opaque block và tính toán chênh lệch tín hiệu hai bên sườn. | Hoàn thiện và test parity cho Comparator ở cả chế độ so sánh (Compare) và trừ (Subtract). |
| **Observer** | `ObserverBlock.java:20-110` | `crates/pumpkin-core/src/block/blocks/redstone/observer.rs` | Phát hiện thay đổi trạng thái khối phía trước, phát xung 2 game tick ra phía sau. | Hoạt động cơ bản đã có, cần đảm bảo thứ tự phát tín hiệu và không tạo vòng lặp cập nhật. | Duy trì và kiểm thử tương thích trong bộ test tích hợp. |

## 2. Quy tắc đối chiếu chi tiết

### 2.1. Quasi-Connectivity (QC)
Mã nguồn Java vanilla:
```java
private boolean getNeighborSignal(final SignalGetter level, final BlockPos pos, final Direction pushDirection) {
   for (Direction direction : Direction.values()) {
      if (direction != pushDirection && level.hasSignal(pos.relative(direction), direction)) {
         return true;
      }
   }
   if (level.hasSignal(pos, Direction.DOWN)) {
      return true;
   }
   BlockPos above = pos.above();
   for (Direction direction : Direction.values()) {
      if (direction != Direction.DOWN && level.hasSignal(above.relative(direction), direction)) {
         return true;
      }
   }
   return false;
}
```

Đối với Dispenser và Dropper:
```java
boolean shouldTrigger = level.hasNeighborSignal(pos) || level.hasNeighborSignal(pos.above());
```

Hành vi BUD (Block Update Detector):
Khi khối nguồn (ví dụ Redstone Block hoặc Redstone Wire) đặt ở vị trí chéo phía trên piston hoặc cách 2 block phía trên, nguồn đó KHÔNG gửi block update trực tiếp đến piston (vì không phải khối kề cạnh piston). Piston giữ nguyên trạng thái chưa bật (chờ kích hoạt). Khi có bất kỳ cập nhật nào ở 6 khối trực tiếp kề cạnh piston (Block update / Neighbor update), piston kiểm tra lại nguồn cấp và phát hiện thấy QC -> lập tức kích hoạt đẩy.

### 2.2. Dây Redstone (Redstone Wire) Strong vs Weak Power
- **Weak Power (Năng lượng yếu)**: Wire cấp năng lượng yếu cho các khối kề cạnh ngang có kết nối và khối bên dưới.
- **Strong Power (Năng lượng mạnh)**: Wire **CHỈ** cấp Strong Power xuống khối nằm trực tiếp phía dưới nó (`args.direction == BlockDirection::Up`). Wire **KHÔNG BAO GIỜ** cấp Strong Power sang các khối nằm ngang cạnh nó.

---
Tài liệu được cập nhật dựa trên khảo sát thực tế source code `Neth` và `jvmc`.
