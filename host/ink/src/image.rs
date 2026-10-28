//! Separable area reduction of premultiplied image pixels. @ref LLP 1149 D2.6.

pub fn reduce(pixels: &[u8], from: [u32; 2], to: [u32; 2]) -> Vec<u8> {
    assert!(from[0] >= to[0] && from[1] >= to[1] && !to.contains(&0));
    let mut horizontal = vec![[0.; 4]; to[0] as usize * from[1] as usize];
    let weights = |at: u32, source: u32, target: u32| {
        let ratio = f64::from(source) / f64::from(target);
        let low = f64::from(at) * ratio;
        let high = f64::from(at + 1) * ratio;
        (low.floor() as u32..high.ceil().min(f64::from(source)) as u32).map(move |i| {
            (
                i,
                (high.min(f64::from(i + 1)) - low.max(f64::from(i))) / ratio,
            )
        })
    };
    for x in 0..to[0] {
        for (sx, weight) in weights(x, from[0], to[0]) {
            for y in 0..from[1] {
                let source = ((y * from[0] + sx) * 4) as usize;
                let dest = &mut horizontal[(y * to[0] + x) as usize];
                for c in 0..4 {
                    dest[c] += f64::from(pixels[source + c]) * weight;
                }
            }
        }
    }
    let mut output = vec![0; to[0] as usize * to[1] as usize * 4];
    let mut row = vec![[0.; 4]; to[0] as usize];
    for y in 0..to[1] {
        row.fill([0.; 4]);
        for (sy, weight) in weights(y, from[1], to[1]) {
            for x in 0..to[0] as usize {
                for c in 0..4 {
                    row[x][c] += horizontal[sy as usize * to[0] as usize + x][c] * weight;
                }
            }
        }
        for (x, pixel) in row.iter().enumerate() {
            for c in 0..4 {
                output[(y as usize * to[0] as usize + x) * 4 + c] =
                    pixel[c].round().clamp(0., 255.) as u8;
            }
        }
    }
    output
}
