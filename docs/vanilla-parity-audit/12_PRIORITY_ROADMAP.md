# 12. PRIORITY ROADMAP TO 100% VANILLA PARITY
## LỘ TRÌNH 10 GIAI ĐOẠN ĐẠT MỨC TƯƠNG ĐỒNG TUYỆT ĐỐI VỚI MINECRAFT 26.3

---

```
Phase 1: Build & Blocker Resolution
   │
Phase 2: Redstone Quasi-Connectivity & Microticks
   │
Phase 3: Villager Brain Schedule & Gossip AI
   │
Phase 4: Dimension Travel & Portal Transitions
   │
Phase 5: Container Ticking & Advanced Crafting
   │
Phase 6: Complete Entity Combat & Status Effects
   │
Phase 7: Raid Progression & Boss Encounters
   │
Phase 8: Data-Driven Datapack Full Integration
   │
Phase 9: End-to-End Differential Verification
   │
Phase 10: Production Release & Stress Testing
```

---

### Chi Tiết Từng Giai Đoạn

#### Giai đoạn 1: Khắc Phục Lỗi Biên Dịch & Nền Tảng (Ưu tiên: BLOCKER)
- Sửa lỗi kiểu `&String` vs `&str` tại `login_acknowledged.rs:83`.
- Đảm bảo `cargo check --workspace` và `cargo test --workspace` chạy sạch 100% không cảnh báo nghiêm trọng.

#### Giai đoạn 2: Hoàn Thiện Cơ Chế Redstone Kỹ Thuật (Ưu tiên: HIGH)
- Thêm kiểm tra Quasi-Connectivity cho Piston, Dropper, Dispenser đối chiếu với `PistonBaseBlock.java` trong `jvmc`.
- Xây dựng hàng đợi Scheduled Tick chuẩn xác cho Redstone Torch và Repeater.

#### Giai đoạn 3: Trí Tuệ Nhân Tạo Dân Làng & Hệ Thống Kinh Tế (Ưu tiên: HIGH)
- Triển khai máy trạng thái lịch trình Dân làng (`WORK`, `MEET`, `REST`).
- Hoàn thiện hệ thống giao dịch nâng cấp cấp bậc (Novice -> Master) và chia sẻ tin đồn (Gossip).

#### Giai đoạn 4: Chuyển Chiều Không Gian (Dimensions & Portals) (Ưu tiên: HIGH)
- Triển khai thuật toán tìm kiếm và tự động tạo khung cổng Nether tại tọa độ đích (tỷ lệ 8:1).
- Hỗ trợ dịch chuyển vào The End (sinh bệ đá obsidian tại tọa độ 100, 49, 0).

#### Giai đoạn 5: Tương Tác Vật Phẩm & Lò Luyện Nâng Cao (Ưu tiên: MEDIUM)
- Hoàn thiện chu trình pha chế thuốc của Giàn Pha Thuốc (`BrewingStandBlockEntity`).
- Triển khai Bàn Rèn (`SmithingTable`) hỗ trợ phôi nâng cấp Netherite và hoa văn giáp.

#### Giai đoạn 6: Chiến Đấu Thực Thể & Hiệu Ứng Trạng Thái (Ưu tiên: MEDIUM)
- Hoàn thiện toàn bộ các hiệu ứng Potion (Invisibility, Glowing, Levitation, Bad Omen, Trial Omen).
- Xác thực hộp va chạm chính xác theo từng tư thế (Bơi, Bò, Lướt cánh Elytra).

#### Giai đoạn 7: Đợt Sóng Cướp Bóc & Trận Đánh Trùm (Ưu tiên: MEDIUM)
- Xây dựng vòng lặp đợt sóng Raid (Raid Waves) khi người chơi có hiệu ứng Bad Omen vào làng.
- Triển khai AI Trùm Rồng Ender (`EnderDragon`) và Wither.

#### Giai đoạn 8: Đầy Đủ Tính Năng Datapack & Registry Tags (Ưu tiên: MEDIUM)
- Nạp động 100% các tệp JSON trong thư mục datapack của world.

#### Giai đoạn 9: Kiểm Thử Vi Sai Tự Động Toàn Diện (Ưu tiên: HIGH)
- Kết nối harness kiểm thử vi sai tự động giữa Neth và JVM Server gốc theo cùng seed và tọa độ.

#### Giai đoạn 10: Đóng Gói Phát Hành & Tối Ưu Tải Cao (Ưu tiên: FINAL)
- Kiểm thử tải 100+ người chơi đồng thời và xác thực tính ổn định bộ nhớ.
