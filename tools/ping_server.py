import socket, struct, json

def read_varint(s):
    val = 0
    for i in range(5):
        b = ord(s.recv(1))
        val |= (b & 0x7F) << (7 * i)
        if not (b & 0x80):
            break
    return val

def write_varint(val):
    out = bytearray()
    while True:
        b = val & 0x7F
        val >>= 7
        if val != 0:
            b |= 0x80
        out.append(b)
        if val == 0:
            break
    return bytes(out)

try:
    s = socket.create_connection(('127.0.0.1', 25565), timeout=3)
    # Handshake packet
    host = b'localhost'
    proto = write_varint(777)
    data = b'\x00' + proto + write_varint(len(host)) + host + struct.pack('>H', 25565) + write_varint(1)
    s.sendall(write_varint(len(data)) + data)
    
    # Status Request packet
    s.sendall(b'\x01\x00')
    
    # Read response
    pkt_len = read_varint(s)
    pkt_id = read_varint(s)
    str_len = read_varint(s)
    raw = b''
    while len(raw) < str_len:
        raw += s.recv(str_len - len(raw))
    res = json.loads(raw.decode('utf-8', errors='replace'))
    print("PING_SUCCESS")
    print("Version Name:", res.get('version', {}).get('name'))
    print("Protocol:", res.get('version', {}).get('protocol'))
    players = res.get('players', {})
    print(f"Players: {players.get('online', 0)}/{players.get('max', 0)}")
    print("MOTD:", res.get('description'))
except Exception as e:
    print("PING_ERROR:", e)
finally:
    s.close()
