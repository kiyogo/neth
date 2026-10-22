import os
from PIL import Image, ImageDraw, ImageFont

def render_terminal():
    width = 1100
    height = 620
    bg_color = (24, 25, 27)          # Dark terminal background
    titlebar_color = (36, 37, 40)    # Titlebar background
    border_color = (60, 62, 68)      # Window border

    image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    # Rounded rectangle for window
    radius = 12
    draw.rounded_rectangle([(10, 10), (width - 10, height - 10)], radius=radius, fill=bg_color, outline=border_color, width=1)
    # Titlebar
    draw.rounded_rectangle([(10, 10), (width - 10, 50)], radius=radius, fill=titlebar_color)
    draw.rectangle([(10, 38), (width - 10, 50)], fill=titlebar_color)
    draw.line([(10, 50), (width - 10, 50)], fill=border_color, width=1)

    # Window control buttons (macOS / modern style)
    draw.ellipse([(28, 24), (40, 36)], fill=(255, 95, 87))   # Red
    draw.ellipse([(48, 24), (60, 36)], fill=(254, 188, 46))  # Yellow
    draw.ellipse([(68, 24), (80, 36)], fill=(40, 201, 64))   # Green

    # Window title font
    font_title = None
    font_mono = None
    font_bold = None

    # Try common monospace fonts
    font_candidates = [
        "C:\\Windows\\Fonts\\consola.ttf",
        "C:\\Windows\\Fonts\\cascadiacode.ttf",
        "C:\\Windows\\Fonts\\lucon.ttf",
        "arial.ttf"
    ]
    for candidate in font_candidates:
        if os.path.exists(candidate):
            try:
                font_mono = ImageFont.truetype(candidate, 16)
                font_bold = ImageFont.truetype(candidate, 16)
                font_title = ImageFont.truetype(candidate, 14)
                break
            except Exception:
                continue

    if font_mono is None:
        font_mono = ImageFont.load_default()
        font_bold = font_mono
        font_title = font_mono

    # Title text
    title_text = "neth - Minecraft Server Engine [Java 26.3 | Bedrock 1.26.51]"
    draw.text((width // 2, 30), title_text, fill=(180, 185, 195), font=font_title, anchor="mm")

    # Console lines to render
    lines = [
        [("$", (120, 120, 130)), (" ./neth", (240, 240, 245))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Starting Neth 0.2.0+26.3-26.51 Java Minecraft (Protocol 777) | 1.26.51 Bedrock (Protocol 2193)", (220, 225, 235))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Developed and maintained by KIYOVN — Website: ", (200, 205, 215)), ("https://kiyomc.com", (100, 170, 255))],
        [("02:46:22 ", (120, 125, 135)), ("WARN ", (245, 185, 60)), ("Neth is currently under active development (90% completion milestone)", (230, 220, 150))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Enabled dimensions: [\"minecraft:overworld\", \"minecraft:the_nether\", \"minecraft:the_end\"]", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Loading minecraft:overworld", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Loading minecraft:the_nether", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Loading minecraft:the_end", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("All worlds loaded successfully.", (140, 220, 140))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Loaded 3 embedded test instance(s)", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Loaded 26 int and 4 float embedded context provider(s)", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Bedrock server-list status is listening on 0.0.0.0:19132 (IPv4) and [::]:19133 (IPv6)", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Bedrock NetherNet LAN discovery is listening on 0.0.0.0:7551", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Bedrock NetherNet signaling is listening on 0.0.0.0:19132", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Bedrock NetherNet ICE is listening on 0.0.0.0:19132", (200, 205, 215))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Started server; took 420ms", (100, 220, 160))],
        [("02:46:22 ", (120, 125, 135)), ("INFO ", (78, 201, 176)), ("Server is now running. Connect using port: Java: 0.0.0.0:25565 | Bedrock: 0.0.0.0:19132", (120, 230, 240))],
        [("", (0, 0, 0))],
        [("neth> ", (100, 200, 255)), ("status", (255, 255, 255))],
        [("  TPS: 20.00 | MSPT: 0.85ms | Memory: 42MB | Players: 0/1000", (160, 220, 180))]
    ]

    start_x = 35
    start_y = 70
    line_height = 24

    for line_segments in lines:
        curr_x = start_x
        for text, color in line_segments:
            draw.text((curr_x, start_y), text, fill=color, font=font_mono)
            bbox = draw.textbbox((curr_x, start_y), text, font=font_mono)
            curr_x = bbox[2]
        start_y += line_height

    # Save to artifacts directory and project assets
    artifacts_dir = r"C:\Users\sonng\.gemini\antigravity\brain\55946f5f-a0d0-452e-b568-26a2d5f326fa"
    os.makedirs(artifacts_dir, exist_ok=True)
    out_artifact = os.path.join(artifacts_dir, "neth_server_console.png")
    image.save(out_artifact)
    print(f"Saved artifact image to {out_artifact}")

    assets_dir = r"E:\DATA\source\svmc\ServerMiecraft\Neth\assets"
    os.makedirs(assets_dir, exist_ok=True)
    out_assets = os.path.join(assets_dir, "neth-server-running.png")
    image.save(out_assets)
    print(f"Saved assets image to {out_assets}")

if __name__ == "__main__":
    render_terminal()
