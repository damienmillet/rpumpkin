import zlib, struct, os

def png_write(path, px):  # px: dict (x,y)->(r,g,b,a)
    raw = b''
    for y in range(64):
        row = b''
        for x in range(64):
            r,g,b,a = px.get((x,y), (0,0,0,0))
            row += bytes((r,g,b,a))
        raw += b'\x00' + row
    def chunk(t, d):
        c = t + d
        return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    data = b'\x89PNG\r\n\x1a\n'
    data += chunk(b'IHDR', struct.pack('>IIBBBBB', 64, 64, 8, 6, 0, 0, 0))
    data += chunk(b'IDAT', zlib.compress(raw, 9))
    data += chunk(b'IEND', b'')
    open(path, 'wb').write(data)

# --- helpers for 64x64 skin layout (front faces) ---
def fill_rect(px, x0, y0, w, h, c):
    for y in range(y0, y0+h):
        for x in range(x0, x0+w):
            px[(x,y)] = c

def noise(px, x0, y0, w, h, c, prob, seed):
    import random
    rnd = random.Random(seed)
    for y in range(y0, y0+h):
        for x in range(x0, x0+w):
            if rnd.random() < prob:
                px[(x,y)] = c

# Blocks (base layer): right leg (0,16), body (16,16), right arm (40,16),
# left leg (16,48), left arm (32,48). Head (0,0)/(32,0) kept transparent.
def full_block(px, bx, by, w, d, h, c):
    fill_rect(px, bx, by, w, h, c)       # top+side strip
    fill_rect(px, bx, by+h, w*d, h, c)   # faces strip

def build(outfit):
    px = {}
    legs_c, legs_dark, boots_c, body_c, body_dark, trim_c, arm_c, skin_c = outfit['palette']
    # legs (h=12, top strip 4px high at y=16 area)
    for (bx, by) in [(0,16), (16,48)]:  # right leg, left leg
        full_block(px, bx, by+4, 12, 4, 12, legs_c)
        fill_rect(px, bx, by, 12, 4, legs_c)
        # boots (bottom 3 rows of front faces)
        for face in range(4):
            fill_rect(px, bx+face*4, by+4+9, 4, 3, boots_c)
        noise(px, bx, by+4, 12, 12, legs_dark, 0.18, bx*7+by)
    # body block (24 wide, 16 tall at 16,16; front at (20,20) 8x12)
    full_block(px, 16, 16+4, 24, 4, 12, body_c)
    fill_rect(px, 16, 16, 24, 4, body_c)
    # arms
    for (bx, by) in [(40,16), (32,48)]:
        full_block(px, bx, by+4, 12, 4, 12, arm_c)
        fill_rect(px, bx, by, 12, 4, arm_c)
        noise(px, bx, by+4, 12, 12, trim_c, 0.10, bx+by*3)
    for deco in outfit.get('deco', []): deco(px)
    return px

def farmer_deco(px):
    # denim overalls on body front (20,20,8,12): blue pants part + straps
    denim = (48,72,140,255); shirt = (198,178,138,255)
    fill_rect(px, 20, 20, 8, 12, shirt)
    fill_rect(px, 20, 26, 8, 6, denim)          # overall bottom
    fill_rect(px, 20, 21, 2, 5, denim)          # left strap
    fill_rect(px, 26, 21, 2, 5, denim)          # right strap
    # wheat ear on chest
    px[(23,23)] = (222,184,60,255); px[(24,24)] = (222,184,60,255)

def miner_deco(px):
    vest = (240,120,20,255); dark = (70,70,78,255); refl = (250,230,90,255)
    fill_rect(px, 20, 20, 8, 12, dark)
    fill_rect(px, 21, 20, 6, 12, vest)          # safety vest
    px[(23,22)] = refl; px[(24,22)] = refl     # reflective strips
    px[(23,26)] = refl; px[(24,26)] = refl
    # grey gloves on arm fronts
    for (bx,by) in [(44,20),(36,52)]:
        fill_rect(px, bx, by+9, 4, 3, (60,60,66,255))

outfits = {
    'fermier': {'palette': ((72,58,36,255),(56,44,26,255),(64,46,26,255),(166,126,72,255),(140,104,58,255),(222,184,60,255),(198,178,138,255),None), 'deco':[farmer_deco]},
    'mineur':  {'palette': ((52,52,60,255),(38,38,44,255),(40,40,46,255),(70,70,78,255),(56,56,64,255),(240,120,20,255),(70,70,78,255),None), 'deco':[miner_deco]},
}
os.makedirs('skins', exist_ok=True)
for name, o in outfits.items():
    png_write(f'skins/metier_{name}.png', build(o))
    print('ok', name)
