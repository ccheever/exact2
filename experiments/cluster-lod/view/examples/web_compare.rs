//! Compare browser canvas PNGs with the native render without an external image package.
use std::{fs::File, io::BufReader};
fn rgba(path: &str) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
    let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    let bytes = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => return Err("expected RGB or RGBA".into()),
    };
    Ok((info.width, info.height, bytes))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let (w, h, a) = rgba(args.get(1).ok_or("browser PNG")?)?;
    let (w2, h2, b) = rgba(args.get(2).ok_or("native PNG")?)?;
    if (w, h) != (w2, h2) {
        return Err("size mismatch".into());
    }
    let mut sum = 0u64;
    let mut max = 0u8;
    let mut above = 0u64;
    let mut changed = 0u64;
    for (a, b) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = [
            a[0].abs_diff(b[0]),
            a[1].abs_diff(b[1]),
            a[2].abs_diff(b[2]),
        ];
        let local = *d.iter().max().unwrap();
        max = max.max(local);
        sum += d.iter().map(|x| *x as u64).sum::<u64>();
        above += u64::from(local > 2);
        changed += u64::from(local > 0);
    }
    println!(
        "{}",
        serde_json::json!({"pixels":w as u64*h as u64,"mean":sum as f64/(w as f64*h as f64*3.0*255.0),"max_byte":max,"fraction_above_2":above as f64/(w as f64*h as f64),"changed_pixels":changed})
    );
    Ok(())
}
