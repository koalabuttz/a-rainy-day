use std::{env, fs, io::BufReader, path::PathBuf};

fn main() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    println!("cargo:rerun-if-changed={}", assets.display());
    // Decode the same PNGs on the build host, including for consoles without std.
    for file in fs::read_dir(assets).expect("asset directory") {
        let path = file.expect("asset entry").path();
        if path.extension().is_none_or(|ext| ext != "png") {
            continue;
        }
        let mut decoder = png::Decoder::new(BufReader::new(
            fs::File::open(&path).expect("open source PNG"),
        ));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().expect("read PNG header");
        let mut data = vec![0; reader.output_buffer_size().expect("PNG buffer size")];
        let info = reader.next_frame(&mut data).expect("decode PNG");
        let width = u16::try_from(info.width).expect("asset width fits in header");
        let height = u16::try_from(info.height).expect("asset height fits in header");
        let mut buffer = Vec::with_capacity(4 + usize::from(width) * usize::from(height) * 4);
        buffer.extend(width.to_le_bytes());
        buffer.extend(height.to_le_bytes());
        for sample in data[..info.buffer_size()].chunks_exact(info.color_type.samples()) {
            buffer.extend(match sample {
                [grey] => [*grey, *grey, *grey, 255],
                [grey, alpha] => [*grey, *grey, *grey, *alpha],
                [r, g, b] => [*r, *g, *b, 255],
                [r, g, b, a] => [*r, *g, *b, *a],
                _ => unreachable!("PNG output has one to four channels"),
            });
        }
        let destination = out
            .join(path.file_name().expect("asset filename"))
            .with_extension("rgba");
        fs::write(destination, buffer).expect("write embedded RGBA");
    }
}
