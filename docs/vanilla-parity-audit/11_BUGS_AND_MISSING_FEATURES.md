# 11. BUGS & MISSING FEATURES REGISTRY (DANH MỤC LỖI & THIẾU SÓT)

Tài liệu này lưu trữ toàn bộ các lỗi xác nhận, lỗi biên dịch, tính năng thiếu và khoảng cách hành vi được phát hiện trong quá trình kiểm toán. Dữ liệu máy đọc có thể tra cứu tại [gap_registry.csv](file:///E:/DATA/source/svmc/ServerMiecraft/Neth/docs/vanilla-parity-audit/gap_registry.csv).

---

### 1. Lỗi Nghiêm Trọng Mức Đột Phá (Blocker / Critical)

#### [BUG-BLOCKER-001] Lỗi Biên Dịch E0308 tại `login_acknowledged.rs`
- **Mức độ:** `BLOCKER` (Ngăn cản server build và chạy)
- **Vị trí:** `crates/pumpkin-core/src/net/java/login/login_acknowledged.rs:44, 49, 54, 59, 64, 69, 76, 83`
- **Hiện tượng:** Compiler báo lỗi `expected &String, found &str` khi truyền kết quả từ closure `sanitize_link` vào `Link::new`.
- **Hành vi Vanilla:** Gửi gói tin `ClientboundServerLinksPacket` chứa các liên kết thông tin server trong trạng thái Configuration.
- **Phương án khắc phục:** Sửa `Link::new` trong `crates/pumpkin-protocol/src/lib.rs` để nhận `&'a str`, hoặc sửa closure trả về `String`.

#### [BUG-WORLD-001] Lỗi Mất Dân Làng & Golem Khi Sinh Làng (Đã Khắc Phục)
- **Mức độ:** `CRITICAL`
- **Vị trí:** `crates/pumpkin-world/src/generation/structure/template/mod.rs`
- **Hiện tượng:** Các ngôi làng sinh ra hoàn toàn không có Dân làng hay Golem.
- **Nguyên nhân:** Tọa độ bị xoay và offset hai lần khiến thực thể văng ra ngoài ranh giới chunk và bị hủy.
- **Trạng thái:** Đã khắc phục và xác nhận pass 1024 trường hợp kiểm thử vi sai.

---

### 2. Các Khoảng Cách Hành Vi Gameplay (Gameplay Gaps)

#### [GAP-RED-001] Thiếu Bán Dẫn Redstone (Quasi-Connectivity)
- **Mức độ:** `HIGH`
- **Vị trí:** `crates/pumpkin-core/src/block/blocks/piston/piston.rs`
- **Hành vi Vanilla:** Piston, Dropper, Dispenser nhận nguồn điện gián tiếp từ khối nằm cách 2 ô phía trên hoặc chéo trên.
- **Hiện trạng Neth:** Chỉ kiểm tra năng lượng từ 6 khối tiếp xúc trực tiếp.

#### [GAP-AI-001] Lịch trình Sinh hoạt Dân làng Chưa Đầy Đủ
- **Mức độ:** `HIGH`
- **Vị trí:** `crates/pumpkin-core/src/entity/mob/villager/`
- **Hành vi Vanilla:** Dân làng tuân thủ nghiêm ngặt thời gian biểu trong ngày (Đi làm -> Gặp gỡ trao đổi tin đồn tại chuông -> Đi ngủ). Tin đồn ảnh hưởng đến giá cả giao dịch.
- **Hiện trạng Neth:** Mới chỉ hỗ trợ nhận nghề qua POI và đi lang thang cơ bản.

#### [GAP-DIM-001] Cổng Chuyển Chiều Nether & End Chưa Hoàn Thiện
- **Mức độ:** `HIGH`
- **Vị trí:** `crates/pumpkin-core/src/world/portal/nether.rs`
- **Hành vi Vanilla:** Đứng trong cổng Nether đếm ngược chuyển chiều không gian, tìm hoặc tạo cổng tương ứng ở tọa độ X/8, Z/8.
- **Hiện trạng Neth:** Mới chỉ kiểm tra tạo khung cổng bằng bật lửa, chưa kích hoạt chuyển chiều thực thể qua runtime.
