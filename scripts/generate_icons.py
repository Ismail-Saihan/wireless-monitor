"""
Application Icon Generator for Wireless Virtual Monitor.
Synthesizes crisp, modern multi-resolution icons for Windows (.ico) and Linux (.png)
using Pillow supersampling.
"""

import os
from PIL import Image, ImageDraw


def generate_master_icon(size=1024):
    # High resolution master canvas with RGBA transparency
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # 1. Background rounded container
    pad = int(size * 0.05)
    corner_radius = int(size * 0.22)
    # Slate dark background with gradient feel
    draw.rounded_rectangle(
        [pad, pad, size - pad, size - pad],
        radius=corner_radius,
        fill=(15, 23, 42, 255),  # Slate 900
        outline=(56, 189, 248, 200),  # Sky 400 accent border
        width=int(size * 0.02),
    )

    # 2. Monitor outer bezel
    m_left = int(size * 0.16)
    m_top = int(size * 0.18)
    m_right = int(size * 0.84)
    m_bottom = int(size * 0.68)
    m_radius = int(size * 0.06)

    draw.rounded_rectangle(
        [m_left, m_top, m_right, m_bottom],
        radius=m_radius,
        fill=(30, 41, 59, 255),  # Slate 800
        outline=(14, 165, 233, 255),  # Sky 500
        width=int(size * 0.018),
    )

    # 3. Monitor inner screen display
    s_pad = int(size * 0.025)
    draw.rounded_rectangle(
        [m_left + s_pad, m_top + s_pad, m_right - s_pad, m_bottom - s_pad],
        radius=int(m_radius * 0.6),
        fill=(8, 14, 26, 255),
    )

    # 4. Monitor stand (neck + base)
    neck_w = int(size * 0.08)
    neck_left = (size - neck_w) // 2
    neck_right = neck_left + neck_w
    neck_top = m_bottom
    neck_bottom = int(size * 0.80)

    draw.rectangle(
        [neck_left, neck_top, neck_right, neck_bottom],
        fill=(71, 85, 105, 255),  # Slate 600
    )

    base_w = int(size * 0.32)
    base_left = (size - base_w) // 2
    base_right = base_left + base_w
    base_top = neck_bottom
    base_bottom = int(size * 0.85)

    draw.rounded_rectangle(
        [base_left, base_top, base_right, base_bottom],
        radius=int(size * 0.02),
        fill=(100, 116, 139, 255),  # Slate 500
        outline=(56, 189, 248, 180),
        width=int(size * 0.008),
    )

    # 5. Glowing wireless waves inside the monitor screen
    center_x = size // 2
    center_y = int(size * 0.43)

    # Small center dot
    dot_r = int(size * 0.022)
    draw.ellipse(
        [center_x - dot_r, center_y - dot_r, center_x + dot_r, center_y + dot_r],
        fill=(56, 189, 248, 255),  # Bright Sky Blue
    )

    # Inner wave arc
    arc1_r = int(size * 0.075)
    draw.arc(
        [center_x - arc1_r, center_y - arc1_r, center_x + arc1_r, center_y + arc1_r],
        start=210,
        end=330,
        fill=(56, 189, 248, 240),
        width=int(size * 0.02),
    )

    # Middle wave arc
    arc2_r = int(size * 0.13)
    draw.arc(
        [center_x - arc2_r, center_y - arc2_r, center_x + arc2_r, center_y + arc2_r],
        start=215,
        end=325,
        fill=(14, 165, 233, 230),
        width=int(size * 0.02),
    )

    # Outer wave arc
    arc3_r = int(size * 0.185)
    draw.arc(
        [center_x - arc3_r, center_y - arc3_r, center_x + arc3_r, center_y + arc3_r],
        start=220,
        end=320,
        fill=(3, 105, 161, 220),
        width=int(size * 0.02),
    )

    return img


def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    icons_dir = os.path.join(base_dir, "src-tauri", "icons")
    os.makedirs(icons_dir, exist_ok=True)

    print(f"Generating master 1024x1024 icon canvas...")
    master = generate_master_icon(1024)

    # Target files to output
    resampling = getattr(Image, "Resampling", Image).LANCZOS

    # 1. 512x512 master icon.png
    icon_512 = master.resize((512, 512), resample=resampling)
    icon_512_path = os.path.join(icons_dir, "icon.png")
    icon_512.save(icon_512_path, "PNG")
    print(f"  -> Generated: {icon_512_path} (512x512)")

    # 2. 128x128@2x.png (256x256)
    icon_256 = master.resize((256, 256), resample=resampling)
    icon_256_path = os.path.join(icons_dir, "128x128@2x.png")
    icon_256.save(icon_256_path, "PNG")
    print(f"  -> Generated: {icon_256_path} (256x256)")

    # 3. 128x128.png
    icon_128 = master.resize((128, 128), resample=resampling)
    icon_128_path = os.path.join(icons_dir, "128x128.png")
    icon_128.save(icon_128_path, "PNG")
    print(f"  -> Generated: {icon_128_path} (128x128)")

    # 4. 32x32.png
    icon_32 = master.resize((32, 32), resample=resampling)
    icon_32_path = os.path.join(icons_dir, "32x32.png")
    icon_32.save(icon_32_path, "PNG")
    print(f"  -> Generated: {icon_32_path} (32x32)")

    # 5. Multi-resolution Windows icon.ico
    # Windows icons support 16, 24, 32, 48, 64, 128, 256
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    ico_images = [master.resize(s, resample=resampling) for s in ico_sizes]
    icon_ico_path = os.path.join(icons_dir, "icon.ico")
    ico_images[0].save(
        icon_ico_path,
        format="ICO",
        sizes=ico_sizes,
        append_images=ico_images[1:],
    )
    print(f"  -> Generated: {icon_ico_path} (Multi-res ICO: {[s[0] for s in ico_sizes]})")

    print("All icon assets synthesized successfully!")


if __name__ == "__main__":
    main()
