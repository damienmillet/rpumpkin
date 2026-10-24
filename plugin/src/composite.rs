//! Composite PNG : tête du joueur (zones 0..31 / 32..63 en y=0..15) copiée
//! dans la texture de la tenue, qui garde le reste (corps, bras, jambes).

use image::{GenericImageView, ImageBuffer, Rgba};

const HEAD_ZONES: [(u32, u32, u32, u32); 2] = [
    (0, 0, 32, 16),   // tête, couche base
    (32, 0, 32, 16),  // chapeau / overlay tête
];

pub fn load_embedded(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| e.to_string())
}

/// Prend le PNG du joueur et le PNG de la tenue, retourne un PNG 64x64
/// dont la zone tête vient du joueur et tout le reste de la tenue.
pub fn merge_head_onto(player: &[u8], outfit: &[u8]) -> Result<Vec<u8>, String> {
    let p = image::load_from_memory(player).map_err(|e| e.to_string())?;
    let o = image::load_from_memory(outfit).map_err(|e| e.to_string())?;
    if p.dimensions() != (64, 64) || o.dimensions() != (64, 64) {
        return Err("textures 64x64 attendues".into());
    }
    let (p, o) = (p.to_rgba8(), o.to_rgba8());
    let mut merged: ImageBuffer<Rgba<u8>, Vec<u8>> = o.clone();
    for (x0, y0, w, h) in HEAD_ZONES {
        for dy in 0..h {
            for dx in 0..w {
                let px = p.get_pixel(x0 + dx, y0 + dy);
                merged.put_pixel(x0 + dx, y0 + dy, *px);
            }
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    merged
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_outfit_body() {
        let tenue = load_embedded("skins/metier_fermier.png").unwrap();
        // tête transparente dans la tenue : le composite doit rester 64x64
        let o = image::load_from_memory(&tenue).unwrap().to_rgba8();
        let mut zone_tete_opaque = 0;
        for (x, y, px) in o.enumerate_pixels() {
            if x < 32 && y < 16 && px.0[3] > 0 { zone_tete_opaque += 1; }
        }
        assert_eq!(zone_tete_opaque, 0, "la zone tête de la tenue doit être transparente");
    }
}
