# 08. GAMEPLAY MECHANICS AUDIT
## ĐỐI CHIẾU CƠ CHẾ GAMEPLAY: REDSTONE, VẬT LÝ, CHIẾN ĐẤU & MÔI TRƯỜNG

---

### 1. Hệ thống Đá Đỏ (Redstone Engine)

Hệ thống Redstone trong Neth đã được triển khai các thành phần nền tảng quan trọng trong `crates/pumpkin-core/src/block/blocks/redstone/`:

#### Các Khối Đã Hoạt động:
1. **Dây Đá Đỏ (`redstone_wire.rs` - 25KB):**
   - Tính toán mức suy giảm năng lượng từ 15 về 0.
   - Cập nhật hướng kết nối (Cross, Dot, Line) tương tác với khối lân cận.
   - Cập nhật tín hiệu theo thứ tự láng giềng `BlockDirection::update_order()`.
2. **Piston & Piston Dính (`piston.rs` - 22KB):**
   - Kiểm tra giới hạn đẩy tối đa 12 khối.
   - Kiểm tra các khối không thể dịch chuyển: Obsidian, Crying Obsidian, Respawn Anchor, Reinforced Deepslate, Khối có Block Entity (`has_block_block_entity`).
   - Xử lý hành vi phá hủy (`PistonBehavior::Destroy`) đối với hoa, đuốc, cỏ.
3. **Bộ Lặp (`repeater.rs`) & Bộ So Sánh (`comparator.rs`):**
   - Độ trễ 1-4 tick đá đỏ (2-8 game ticks).
   - Chế độ so sánh và chế độ trừ tín hiệu của Comparator dựa trên dung lượng hòm đồ.
4. **Observer (`observer.rs`), Đuốc Đá Đỏ (`redstone_torch.rs`), Đèn Đá Đỏ (`redstone_lamp.rs`).**

#### Khoảng trống Cần Bổ sung (Redstone Gaps):
- **Quasi-Connectivity (QC):** Neth hiện tại **chưa hỗ trợ Quasi-Connectivity**. Trong Vanilla Java, Piston/Dispenser/Dropper có thể nhận năng lượng từ khối nằm chéo phía trên hoặc cách 2 khối theo trục Y. Đây là thiếu sót lớn nhất đối với cộng đồng kỹ thuật Redstone.
- **Hàng đợi Vi-Tick (Redstone Microtick Queue):** Vanilla Java sử dụng danh sách ưu tiên băm để xác định chính xác thứ tự nổ mạch trong cùng 1 game tick; Neth hiện dùng tick scheduler thông thường.

---

### 2. Chất Lỏng & Vật Lý Chuyển Động (Fluids & Physics)

- **Lan truyền Nước & Dung Nham:**
  - Tính toán độ cao dòng chảy, tạo nguồn nước vô hạn (2 nguồn kề nhau thành nguồn thứ 3).
  - Tương tác nước chạm dung nham: Tạo đá cuội (Cobblestone) khi dòng chảy chạm nhau; tạo Hắc diện thạch (Obsidian) khi nước chạm nguồn dung nham.
- **Va Chạm Hình Hộp (AABB Collision):**
  - Hệ thống va chạm VoxelShape tính toán lực cản khối hộp, lực nổi của thực thể dưới nước (`buoyancy`), sát thương ngạt thở (`suffocation`) và sát thương rơi tự do (`fall_damage`).

---

### 3. Cơ Chế Chiến Đấu & Sát Thương (Combat & Damage)

- **Nguồn Sát Thương (Damage Types):** Hỗ trợ đầy đủ các loại sát thương chuẩn 1.21 / 26.3 (Tấn công cận chiến, Bắn cung, Nổ TNT, Cháy lửa, Rơi từ trên cao, Ngạt nước, Đòn đập Chùy Mace AOE).
- **Công thức Giảm Sát Thương của Giáp:** Tính toán chính xác theo công thức Vanilla:
  $$\text{Defense} = \text{armor} - \frac{4 \times \text{damage}}{8 + \text{toughness}}$$
- **Thời gian Hồi Đòn (Attack Cooldown) & Bật Nảy (Knockback):** Đồng bộ chính xác với thanh hồi vũ khí client.
