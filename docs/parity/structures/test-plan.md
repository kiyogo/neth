# Structures Parity Differential Test Plan & Matrix

**Dự án**: Neth (Minecraft Server in Rust)  
**Phiên bản tham chiếu**: Minecraft Java 26.3 (`jvmc`)  
**Tài liệu**: Kế hoạch kiểm thử vi sai và ma trận kiểm chứng cấu trúc thế giới  

---

## 1. MỤC TIÊU VÀ PHƯƠNG PHÁP KIỂM THỬ VI SAI (DIFFERENTIAL TESTING)

Mục tiêu cốt lõi của chiến lược kiểm thử vi sai là sử dụng Minecraft Java Edition gốc (`jvmc`) làm **Oracle (chân lý tham chiếu)** để so sánh trực tiếp kết quả sinh cấu trúc của Neth trên cùng một cấu hình đầu vào:
- **Seed thế giới**: Seed cố định và ngẫu nhiên.
- **Tọa độ Chunk**: Từng cặp `(chunk_x, chunk_z)` cụ thể.
- **Kích thước và Bounding Box**: Khớp chính xác tới từng block `[min_x, min_y, min_z, max_x, max_y, max_z]`.
- **Số lượng pieces và kiểu piece**: Khớp thứ tự và cấu trúc dữ liệu cây piece.
- **Biến dạng địa hình và trọng lực (Terrain Gravity & Height Projection)**: Khớp độ cao bề mặt đường làng và móng công trình.

---

## 2. MA TRẬN KIỂM THỬ (TEST MATRIX)

| Nhóm Kiểm thử | Cấu trúc bao phủ | Oracle / Fixture | Tiêu chuẩn Đánh giá | Trạng thái |
|:---|:---|:---|:---|:---|
| **Road Gravity 1024** | `village_plains`, `village_taiga`, `village_savanna`, `village_snowy`, `village_desert` | `village_path_tests.rs` | Khớp 1024/1024 trường hợp tính toán độ cao đường làng sau khi áp dụng carving/surface | `PASSED` |
| **Structure Starts Dump** | 19 cấu trúc chính trên cả 3 chiều không gian (Overworld, Nether, End) | `assets/tests/structure_starts.json` (2.07 MB) | Khớp từng start, piece count, piece bounding box, ground level delta, junctions | `PASSED` |
| **SinglePiece Sea Level** | `desert_pyramid`, `jungle_pyramid` | Unit tests mô phỏng địa hình | Từ chối sinh nếu `lowest_y < 63`; Sinh đúng chunk center `(x + 8, y, z + 8)` khi trên cạn | `PASSED` |
| **5x5 Box Corner Sampling** | `mansion`, `end_city` | Sampler tracking calls | Đúng 4 góc xoay theo rotation; Từ chối nếu Y < 60 | `PASSED` |
| **Nether Fossils** | `nether_fossil` | Nether column sampler test | Không bao giờ sinh trên trần Nether; Dừng tại khối soul sand hoặc khối rắn hợp lệ | `PASSED` |
| **Ocean Monument Graph** | `monument` | `graph.rs` & `mod.rs` | Sinh đầy đủ 58 phòng liên thông, phòng lõi kho báu vàng và Elder Guardian NBT | `PASSED` |
| **Stronghold Concentric** | `stronghold` | Stronghold library & placement tests | Tìm đúng vòng concentric rings, sinh phòng cổng End hợp lệ | `PASSED` |
| **Jigsaw Graphs & Pools** | `ancient_city`, `pillager_outpost`, `trial_chambers`, `trail_ruins` | Template processor tests | Khớp trọng số template pools, alias resolution, rigid element placement | `PASSED` |

---

## 3. BỘ DỮ LIỆU ORACLE (`structure_starts.json`)

Bộ dữ liệu `assets/tests/structure_starts.json` được kết xuất từ server Java 26.3 nguyên bản, quét trên phạm vi hàng nghìn chunk qua cả ba dimension:
1. **Overworld**:
   - `minecraft:trial_chambers`
   - `minecraft:ruined_portal_mountain`
   - `minecraft:abandoned_camp_old_growth_birch_forest`
   - `minecraft:ocean_ruin_cold`
   - `minecraft:monument`
   - `minecraft:village_plains`
   - `minecraft:stronghold`
   - `minecraft:ruined_portal`
   - `minecraft:mineshaft`
   - `minecraft:ruined_portal_ocean`
   - `minecraft:shipwreck`
   - `minecraft:trail_ruins`
   - `minecraft:abandoned_camp_swamp`
   - `minecraft:buried_treasure`
2. **The Nether**:
   - `minecraft:ruined_portal_nether`
   - `minecraft:fortress`
   - `minecraft:bastion_remnant`
   - `minecraft:nether_fossil`
3. **The End**:
   - `minecraft:end_city`

---

## 4. QUY TRÌNH HỒI QUY TỰ ĐỘNG (CI / REGRESSION PREVENTION)

Mọi thay đổi trong module thế giới hoặc generator phải chạy toàn bộ chuỗi kiểm thử sau:
```bash
# 1. Kiểm thử toàn bộ module structures
cargo test -p pumpkin-world --lib generation::structure

# 2. Kiểm thử vi sai với Oracle Minecraft Java 26.3
cargo test -p pumpkin-world --lib proto_chunk_test::structure_starts

# 3. Kiểm thử 1024 trường hợp đường làng
cargo test -p pumpkin-world --lib village_path
```
Bất kỳ sai khác nào về bounding box dù chỉ 1 block cũng sẽ làm fail build ngay lập tức, ngăn ngừa triệt để lỗi hồi quy logic.
