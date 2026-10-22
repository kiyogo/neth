# 07. BLOCKS, ITEMS & INVENTORY SYSTEM AUDIT
## ĐỐI CHIẾU KHỐI, VẬT PHẨM, HÒM ĐỒ & CHẾ TẠO VỚI MINECRAFT 26.3

---

### 1. Khối & Trạng thái Khối (Blocks & Block States)

- **Tổng số loại khối Vanilla:** **864 khối** (tính theo định danh duy nhất trong `BlockIds.java` và `BlockItemIds.java`).
- **Hỗ trợ Dữ liệu Metadata:** **100% (864 khối)** được định nghĩa đầy đủ thuộc tính trạng thái (facing, powered, open, waterlogged, age, stage...) trong `crates/pumpkin-data`.
- **Khối có Logic Tương tác Riêng:** **203 khối** được viết logic tùy biến chuyên sâu trong `crates/pumpkin-core/src/block/blocks/`.
- **Block Entities:** **48 / 49 loại Block Entity** trong Vanilla (`BlockEntityTypeIds.java`) đã được triển khai struct trong `crates/pumpkin-core/src/block/entities/` (Rương, Lò nung, Phễu, Khung tên, Biển báo, Khối lệnh, Khối cấu trúc, Đầu lâu, Lò luyện, Nồi trang trí, Bàn pha thuốc...).

---

### 2. Vật phẩm & Kho Đồ (Items & Inventory System)

- **Tổng số vật phẩm Vanilla:** **1,053 vật phẩm** (`ItemIds.java`).
- **Triển khai Hành vi Đặc thù:** **55 loại vật phẩm** có logic click/sử dụng riêng (Thẻ tên, Cần câu cá, Chùy Mace Smash Attack, Xô nước/dung nham, Bình thuốc ném, Chai kinh nghiệm, Sáp tổ ong...).
- **Hệ thống Quản lý Cửa sổ Hòm đồ (`crates/pumpkin-inventory`):**
  - Hỗ trợ đầy đủ các thao tác click chuột: Chuột trái, Chuột phải, Shift-Click (chuyển đồ nhanh), Kéo thả phân phối (Drag split), Double-click (thu gom cùng loại), Phím số Hotbar Swap, Vứt đồ (Drop key Q).
  - Đồng bộ hóa trạng thái giao dịch kho đồ với client thông qua các gói tin `CContainerSetSlot`, `CContainerSetContent`, xác thực tránh dupe đồ.

---

### 3. Hệ thống Công thức Chế tạo (Recipes Engine)

- **Bảng dữ liệu Công thức:** Tệp `crates/pumpkin-data/src/generated/recipes.rs` (dung lượng 813,512 bytes) lưu trữ hàng nghìn công thức chuẩn của Minecraft.
- **Thuật toán Khớp Công thức (`recipe_matching.rs`):**
  - Kiểm tra công thức có hình dạng (`ShapedRecipe`) và không hình dạng (`ShapelessRecipe`).
  - Hỗ trợ so khớp chính xác theo Tag (ví dụ: `#minecraft:planks` khớp với mọi loại ván gỗ).
  - Vượt qua toàn bộ unit test đối sánh tên chính xác trong `crates/pumpkin-data/tests/recipe_matching.rs`.
- **Hệ thống Nung nấu & Luyện kim:**
  - Lò nung (`FurnaceBlockEntity`), Lò luyện kim (`BlastFurnace`), Lò hun khói (`Smoker`) đã có logic tick tiêu thụ nhiên liệu và trả về sản phẩm.
