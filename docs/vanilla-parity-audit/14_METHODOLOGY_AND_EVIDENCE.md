# 14. METHODOLOGY & AUDIT EVIDENCE
## CÔNG THỨC TOÁN HỌC, TIÊU CHUẨN ĐÁNH GIÁ & NGUYÊN TẮC BẰNG CHỨNG

---

### 1. Công Thức Tính Các Chỉ Số Đo Lường Cốt Lõi

Toàn bộ các số liệu thống kê trong cuộc kiểm toán này được tính toán tự động bằng chương trình từ tập dữ liệu gồm **2,561 yêu cầu và tính năng hành vi độc lập** được lưu trữ tại `feature_inventory.json`:

#### 1.1. Implementation Coverage (Mức Độ Triển Khai)
Đo lường mức độ các yêu cầu chức năng đã được viết mã nguồn trong Neth:
$$\text{Implementation Coverage} = 100 \times \frac{\sum_{i=1}^{N} (w_i \times \text{impl}_i)}{\sum_{i=1}^{N} w_i} = \mathbf{61.50\%}$$

Trong đó:
- $N = 2,561$ là tổng số yêu cầu hành vi đã được kiểm kê.
- $w_i = 1.0$ là trọng số chuẩn mực cho mỗi yêu cầu độc lập.
- Hệ số triển khai $\text{impl}_i$ được chấm nghiêm ngặt theo trạng thái:
  - $\text{impl}_i = 1.0$: Nếu tính năng đã được triển khai đầy đủ logic (`VERIFIED_EQUIVALENT` hoặc `IMPLEMENTED_UNVERIFIED`).
  - $\text{impl}_i = 0.5$: Nếu tính năng được triển khai một phần (`PARTIAL`).
  - $\text{impl}_i = 0.2$: Nếu tính năng mới chỉ có khai báo cấu trúc hoặc stub (`STUB_ONLY`).
  - $\text{impl}_i = 0.0$: Nếu chưa có mã nguồn triển khai (`NOT_IMPLEMENTED`).

#### 1.2. Verified Vanilla Parity (Độ Tương Đồng Đã Kiểm Chứng)
Đo lường tỷ lệ các tính năng đã được xác minh bằng bằng chứng kiểm thử thực nghiệm (Unit Tests, Differential Testing, Oracle Dumps) là chạy đúng 100% với Minecraft Java Edition gốc:
$$\text{Verified Vanilla Parity} = 100 \times \frac{\sum_{i=1}^{N} (w_i \times \text{ver}_i)}{\sum_{i=1}^{N} w_i} = \mathbf{2.03\%}$$

Trong đó:
- $\text{ver}_i = 1.0$: Khi tính năng đã vượt qua bài kiểm thử vi sai tự động hoặc integration test đối chiếu (`VERIFIED_EQUIVALENT`).
- $\text{ver}_i = 0.0$: Đối với tất cả các trạng thái còn lại (kể cả khi đã có code đầy đủ nhưng chưa chạy test kiểm chứng).

#### 1.3. Assessment Coverage (Tỷ Lệ Đánh Giá)
Đo lường tỷ lệ các yêu cầu đã thu thập đủ bằng chứng mã nguồn rõ ràng để đưa ra kết luận:
$$\text{Assessment Coverage} = 100 \times \frac{\text{Số yêu cầu đã đối chiếu}}{\text{Tổng số yêu cầu}} = \mathbf{100.00\%}$$

---

### 2. Tiêu Chuẩn Phân Loại Mức Độ Tin Cậy (Confidence Levels)

- `HIGH`: Đã kiểm chứng trực tiếp bằng mã nguồn thực tế và kết quả test tự động chạy thành công trên workspace.
- `MEDIUM`: Đã phân tích call graph và luồng logic tương ứng nhưng chưa có test tự động độc lập.
- `LOW`: Chỉ có khai báo kiểu dữ liệu hoặc phụ thuộc vào module chưa hoàn thiện.
- `UNKNOWN`: Không có cơ sở kết luận (trong báo cáo này không có mục nào bị xếp vào UNKNOWN).

---

### 3. Quy Tắc Truy Vết Bằng Chứng Mã Nguồn (Code Traceability Rules)

Mọi đánh giá trong bộ báo cáo đều dẫn chứng trực tiếp tới các tệp mã nguồn có thật trong hai repository:
1. **Repository Tham chiếu Minecraft Java (`jvmc`):**
   - Đường dẫn gốc: `E:\DATA\source\svmc\ServerMiecraft\jvmc\src\net\minecraft`
   - Chỉ dẫn chính xác tới từng class, method và hằng số của Mojang.
2. **Repository Kiểm toán Neth Rust Server (`Neth`):**
   - Đường dẫn gốc: `E:\DATA\source\svmc\ServerMiecraft\Neth\crates`
   - Chỉ dẫn chính xác tới từng crate, module, struct, function và dòng code Rust.
