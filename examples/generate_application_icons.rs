//! Regenerate the application's raster assets from its canonical SVG logo.
//! Run through scripts/distribution-icon.ps1; worker icons are independent.

use gpui::{size, DevicePixels, ParsedSvg, SvgRenderer, SvgSize};
use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::{ExtendedColorType, RgbaImage};
use std::error::Error;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let icons = root.join("assets/icons");
    let destination = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| icons.join("oxyde.ico"));
    if args.next().is_some() {
        return Err("Usage: generate_application_icons [project-root] [ico-destination]".into());
    }

    let renderer = SvgRenderer::new(Arc::new(()));
    let svg = renderer.parse_svg(&fs::read(icons.join("logo_oxyde.svg"))?)?;
    rasterize(&renderer, &svg, 600)?.save(icons.join("logo.png"))?;
    // In-app copies sized for 2x displays: GPUI downsamples images in a single
    // GPU pass, so a 600 px source aliases badly at title-bar size.
    rasterize(&renderer, &svg, 40)?.save(icons.join("logo_title.png"))?;
    rasterize(&renderer, &svg, 144)?.save(icons.join("logo_about.png"))?;

    let mut frames = Vec::new();
    for dimension in [16, 20, 24, 32, 40, 48, 64, 128, 256] {
        let image = rasterize(&renderer, &svg, dimension)?;
        frames.push(IcoFrame::as_png(
            image.as_raw(),
            dimension,
            dimension,
            ExtendedColorType::Rgba8,
        )?);
    }
    IcoEncoder::new(File::create(&destination)?).encode_images(&frames)?;
    // Linux packages, for desktops that do not draw the SVG.
    for dimension in [16, 24, 32, 48, 64, 128, 256, 512] {
        let folder = root.join(format!("packaging/linux/icons/{dimension}x{dimension}"));
        fs::create_dir_all(&folder)?;
        rasterize(&renderer, &svg, dimension)?.save(folder.join("io.github.Carlton2001.Oxyde.png"))?;
    }
    println!(
        "Generated {} and {}",
        icons.join("logo.png").display(),
        destination.display()
    );
    Ok(())
}

fn rasterize(
    renderer: &SvgRenderer,
    svg: &ParsedSvg,
    dimension: u32,
) -> Result<RgbaImage, Box<dyn Error>> {
    // Supersampling keeps small icons smooth; every size starts from the vector.
    let pixels = DevicePixels((dimension * 4) as i32);
    let rendered = renderer.render_parsed(svg, SvgSize::ExactSize(size(pixels, pixels)))?;
    let mut rgba = rendered
        .as_bytes(0)
        .ok_or("SVG renderer returned no frame")?
        .to_vec();
    // GPUI exposes straight-alpha BGRA; PNG and ICO encoders expect RGBA.
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let image = RgbaImage::from_raw(dimension * 4, dimension * 4, rgba)
        .ok_or("SVG renderer returned invalid dimensions")?;
    Ok(image::imageops::resize(
        &image,
        dimension,
        dimension,
        image::imageops::FilterType::Lanczos3,
    ))
}
