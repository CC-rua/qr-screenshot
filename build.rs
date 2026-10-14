use std::{env, fs, io::Cursor, path::PathBuf, process::Command};

const ICON_SVG: &str = "assets/qr-scanner-icon-32.svg";
const ICON_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed={ICON_SVG}");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let svg = fs::read(ICON_SVG).expect("application icon SVG must exist in assets");
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())
        .expect("application icon SVG must be valid");

    let mut ico_images = Vec::new();
    for size in ICON_SIZES {
        let png = render_png(&tree, *size);
        fs::write(out_dir.join(format!("app_icon_{size}.png")), &png)
            .expect("write rasterized application icon");
        ico_images.push((*size, png));
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon(&out_dir, &ico_images);
    }
}

fn render_png(tree: &resvg::usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size, size).expect("allocate rasterized application icon");
    let svg_size = tree.size();
    let scale = resvg::tiny_skia::Transform::from_scale(
        size as f32 / svg_size.width(),
        size as f32 / svg_size.height(),
    );
    resvg::render(tree, scale, &mut pixmap.as_mut());

    // tiny-skia stores premultiplied alpha; PNG RGBA channels must be unpremultiplied.
    let rgba = pixmap
        .data()
        .chunks_exact(4)
        .flat_map(|pixel| {
            let alpha = pixel[3] as u16;
            if alpha == 0 {
                [0, 0, 0, 0]
            } else {
                [
                    ((pixel[0] as u16 * 255 / alpha).min(255)) as u8,
                    ((pixel[1] as u16 * 255 / alpha).min(255)) as u8,
                    ((pixel[2] as u16 * 255 / alpha).min(255)) as u8,
                    pixel[3],
                ]
            }
        })
        .collect::<Vec<_>>();
    let image = image::RgbaImage::from_raw(size, size, rgba).expect("valid RGBA icon buffer");
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut png, image::ImageFormat::Png)
        .expect("encode icon PNG");
    png.into_inner()
}

fn embed_windows_icon(out_dir: &std::path::Path, images: &[(u32, Vec<u8>)]) {
    let ico_path = out_dir.join("app_icon.ico");
    let rc_path = out_dir.join("app_icon.rc");
    let res_path = out_dir.join("app_icon.res");
    fs::write(&ico_path, images_as_ico(images)).expect("write generated ICO resource");

    let icon_path = ico_path.to_string_lossy().replace('\\', "\\\\");
    fs::write(&rc_path, format!("1 ICON \"{icon_path}\"\n"))
        .expect("write Windows resource script");

    let Some(rc) = find_resource_compiler() else {
        println!(
            "cargo:warning=Windows SDK rc.exe was not found; the executable file icon will not be embedded."
        );
        return;
    };
    let status = Command::new(rc)
        .arg("/nologo")
        .arg(format!("/fo{}", res_path.display()))
        .arg(&rc_path)
        .status()
        .expect("start Windows resource compiler");
    assert!(status.success(), "Windows resource compiler failed");
    println!("cargo:rustc-link-arg={}", res_path.display());
}

fn images_as_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let directory_size = 6 + images.len() * 16;
    let mut offset = directory_size as u32;
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(images.len() as u16).to_le_bytes());
    for (size, png) in images {
        let dimension = if *size == 256 { 0 } else { *size as u8 };
        ico.extend_from_slice(&[dimension, dimension, 0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        ico.extend_from_slice(png);
    }
    ico
}

fn find_resource_compiler() -> Option<PathBuf> {
    if let Some(path) = env::var_os("RC") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc.exe"));
    }

    let kits_root = env::var_os("KitsRoot10").map(PathBuf::from).or_else(|| {
        env::var_os("ProgramFiles(x86)").map(|path| PathBuf::from(path).join("Windows Kits\\10"))
    })?;
    let target = env::var("TARGET").unwrap_or_default();
    let arch = if target.starts_with("aarch64-") {
        "arm64"
    } else if target.starts_with("i686-") {
        "x86"
    } else {
        "x64"
    };
    let bin_dir = kits_root.join("bin");
    let mut versions = fs::read_dir(bin_dir)
        .ok()?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    versions.sort_by_key(|entry| entry.file_name());
    versions
        .into_iter()
        .rev()
        .map(|entry| entry.path().join(arch).join("rc.exe"))
        .find(|path| path.is_file())
}
