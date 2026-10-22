# 03. NETWORK ENGINE & MINECRAFT PROTOCOL AUDIT
## ĐỐI CHIẾU MẠNG & GIAO THỨC MINECRAFT 26.3 (PROTOCOL 777)

---

### 1. Kiến trúc Tầng Mạng Neth

Tầng mạng của Neth được xây dựng trên nền tảng Tokio bất đồng bộ (asynchronous I/O), chia thành hai crate chuyên biệt:
1. `crates/pumpkin-protocol`: Chịu trách nhiệm serialize, deserialize, framing, compression, encryption và multi-version decoding.
2. `crates/pumpkin-core/src/net`: Chịu trách nhiệm xử lý logic nghiệp vụ cho từng gói tin nhận được (Packet Handlers).

#### Các Thông số Kỹ thuật Chuẩn:
- **Protocol Version:** 777 (`V_26_3`)
- **VarInt / VarLong Encoding:** Triển khai bit-mask tối ưu chuẩn Mojang LEB128 trong `crates/pumpkin-protocol/src/codec/var_int.rs`.
- **Packet Compression:** Hỗ trợ zlib/flate2 với ngưỡng nén cấu hình `compression_threshold` (mặc định 256 bytes).
- **Packet Encryption:** Chuẩn mã hóa dòng AES-128-CFB8 với khóa chia sẻ trao đổi qua RSA-1024 (`crates/pumpkin-protocol/src/codec/`).
- **Framing Limit:** `MAX_PACKET_SIZE = 2,097,152` bytes (2MB), `MAX_PACKET_DATA_SIZE = 8,388,608` bytes (8MB).

---

### 2. Thống kê Toàn diện Các Trạng thái Giao thức (Protocol States)

| State | Gói tin JVM (`jvmc`) | Đã Encode/Decode trong Neth | Đã có Handler Xử lý | Tỷ lệ Hoàn thiện |
| :--- | :---: | :---: | :---: | :---: |
| **Handshake** | 3 | 3 | 3 | **100.00%** |
| **Status** | 5 | 5 | 5 | **100.00%** |
| **Login** | 12 | 12 | 11 | **91.67%** |
| **Configuration** | 12 | 12 | 10 | **83.33%** |
| **Play (Serverbound)** | 78 | 78 | 75 | **96.15%** |
| **Play (Clientbound)** | 149 | 149 | N/A (Gửi đi) | **100.00%** |
| **Ping / Cookie / Common** | 39 | 35 | 32 | **82.05%** |
| **TỔNG CỘNG** | **231** | **226** | **150** | **64.85% (Coverage)** |

---

### 3. Phân tích Chi tiết Lỗi BLOCKER Biên dịch tại `login_acknowledged.rs`

- **Vị trí tệp:** `crates/pumpkin-core/src/net/java/login/login_acknowledged.rs:83`
- **Mã lỗi Rust:** `error[E0308]: mismatched types`
- **Mô tả:**
  ```text
  links.push(Link::new(
      Label::TextComponent(TextComponent::text(key.clone()).into()),
      sanitize_link(value), // expected `&String`, found `&str`
  ));
  ```
- **Nguyên nhân gốc rễ (Root Cause):**
  Trong `crates/pumpkin-protocol/src/lib.rs:529`, constructor `Link::new` được định nghĩa là:
  ```rust
  pub const fn new(label: Label, url: &'a String) -> Self
  ```
  Trong khi đó, closure `sanitize_link` được viết như sau:
  ```rust
  let sanitize_link = |link: &str| -> &str { ... };
  ```
  Việc truyền kết quả kiểu `&str` vào tham số mong đợi `&String` vi phạm hệ thống kiểu của Rust.
- **Tác động:** Khiến toàn bộ dự án Neth không thể biên dịch thành công qua `cargo check` hoặc `cargo build`.
- **Giải pháp khắc phục:**
  Sửa định nghĩa `Link::new` trong `crates/pumpkin-protocol/src/lib.rs` để nhận `&'a str` (chuẩn idiom của Rust):
  ```rust
  pub const fn new(label: Label, url: &'a str) -> Self
  ```
  Hoặc tại `login_acknowledged.rs`, truyền chuỗi sở hữu hoặc tham chiếu tương thích.

---

### 4. Danh sách Các Serverbound Play Packets Chưa Có Handler Đầy Đủ

1. `ServerboundDebugSubscriptionRequestPacket`: Chưa có handler trong Neth (bỏ qua silent).
2. `ServerboundPaddleBoatPacket`: Handler chỉ nhận gói tin nhưng chưa cập nhật trạng thái quạt chèo đồng bộ cho các người chơi khác.
3. `ServerboundJigsawGeneratePacket`: Handler mới chỉ phân quyền OP, chưa gọi hàm sinh Jigsaw trực tiếp tại runtime.
