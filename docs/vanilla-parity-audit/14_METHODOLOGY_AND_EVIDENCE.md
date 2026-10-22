# 14. METHODOLOGY & AUDIT EVIDENCE
## CÔNG THỨC TOÁN HỌC, TIÊU CHUẨN ĐÁNH GIÁ & NGUYÊN TẮC BẰNG CHỨNG

---

### 1. Công Thức Tính Chỉ Số Đo Lường

Mọi số liệu trong báo cáo này được tính toán tự động bằng chương trình kiểm toán tĩnh từ tập dữ liệu gồm **2,561 yêu cầu hành vi chi tiết**:

$$\text{Implementation Coverage} = 100 \times \frac{\sum_{i} (w_i \times \text{impl}_i)}{\sum_{i} w_i} = 60.68\%$$

$$\text{Verified Vanilla Parity} = 100 \times \frac{\sum_{i} (w_i \times \text{ver}_i)}{\sum_{i} w_i} = 0.20\%$$

Trong đó:
- $w_i = 1.0$ là trọng số chuẩn mực cho từng yêu cầu độc lập.
- $\text{impl}_i = 1.0$ nếu tính năng có mã nguồn triển khai đầy đủ; $0.5$ nếu là một phần (`PARTIAL`); $0.2$ nếu là khai báo stub (`STUB_ONLY`); $0.0$ nếu chưa triển khai (`NOT_IMPLEMENTED`).
- $\text{ver}_i = 1.0$ khi tính năng đã được kiểm chứng tương đương bằng kiểm thử vi sai tự động hoặc integration test đã vượt qua; $0.0$ nếu chưa kiểm chứng (`UNVERIFIED`).

---

### 2. Quy Tắc Truy Vết Bằng Chứng (Code Traceability Rule)

Không kết luận bất kỳ tính năng nào mà không chỉ rõ đường dẫn tệp thực tế trong cả hai repository:
- **Minecraft Java:** Luôn chỉ rõ class, phương thức hoặc tệp tài nguyên JSON tại `E:\DATA\source\svmc\ServerMiecraft\jvmc\src\...`.
- **Neth Rust:** Luôn chỉ rõ crate, module, hàm hoặc struct tại `E:\DATA\source\svmc\ServerMiecraft\Neth\crates\...`.
