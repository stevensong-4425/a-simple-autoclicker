#[path = "src/icon.rs"]
mod icon;

use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        println!("cargo:warning=Executable icon embedding currently requires the MSVC target");
        return;
    }

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR"));
    let icon_path = output_dir.join("a-simple-autoclicker.ico");
    let resource_script = output_dir.join("a-simple-autoclicker.rc");
    let compiled_resource = output_dir.join("a-simple-autoclicker.res");

    fs::write(&icon_path, windows_icon()).expect("could not write the generated Windows icon");
    fs::write(&resource_script, "1 ICON \"a-simple-autoclicker.ico\"\r\n")
        .expect("could not write the Windows resource script");

    let resource_compiler = find_resource_compiler().unwrap_or_else(|| {
        panic!("Could not find rc.exe. Install the Windows 10 or 11 SDK through Visual Studio.")
    });
    let status = Command::new(resource_compiler)
        .current_dir(&output_dir)
        .args([
            "/nologo",
            "/fo",
            compiled_resource
                .file_name()
                .expect("compiled resource has no filename")
                .to_str()
                .expect("compiled resource path is not UTF-8"),
            resource_script
                .file_name()
                .expect("resource script has no filename")
                .to_str()
                .expect("resource script path is not UTF-8"),
        ])
        .status()
        .expect("could not start the Windows resource compiler");
    assert!(status.success(), "the Windows resource compiler failed");

    println!(
        "cargo:rustc-link-arg-bin=a-simple-autoclicker={}",
        compiled_resource.display()
    );
}

fn find_resource_compiler() -> Option<PathBuf> {
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc.exe"));
    }

    let program_files = env::var_os("ProgramFiles(x86)")?;
    let sdk_bin = Path::new(&program_files).join("Windows Kits").join("10").join("bin");
    let mut versions = fs::read_dir(sdk_bin)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    versions.sort_by_key(|entry| entry.file_name());
    versions.reverse();

    versions
        .into_iter()
        .map(|entry| entry.path().join("x64").join("rc.exe"))
        .find(|candidate| candidate.is_file())
}

fn windows_icon() -> Vec<u8> {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let images = sizes
        .into_iter()
        .map(|size| (size, dib_image(size)))
        .collect::<Vec<_>>();
    let image_offset = 6 + images.len() * 16;
    let mut output = Vec::with_capacity(
        image_offset + images.iter().map(|(_, image)| image.len()).sum::<usize>(),
    );

    push_u16(&mut output, 0);
    push_u16(&mut output, 1);
    push_u16(&mut output, images.len() as u16);

    let mut next_offset = image_offset as u32;
    for (size, image) in &images {
        output.push(if *size == 256 { 0 } else { *size as u8 });
        output.push(if *size == 256 { 0 } else { *size as u8 });
        output.extend_from_slice(&[0, 0]);
        push_u16(&mut output, 1);
        push_u16(&mut output, 32);
        push_u32(&mut output, image.len() as u32);
        push_u32(&mut output, next_offset);
        next_offset += image.len() as u32;
    }
    for (_, image) in images {
        output.extend_from_slice(&image);
    }

    output
}

fn dib_image(size: u32) -> Vec<u8> {
    let rgba = icon::mouse_rgba(size, false);
    let mask_stride = size.div_ceil(32) * 4;
    let bitmap_bytes = size * size * 4;
    let mask_bytes = mask_stride * size;
    let mut image = Vec::with_capacity((40 + bitmap_bytes + mask_bytes) as usize);

    push_u32(&mut image, 40);
    push_u32(&mut image, size);
    push_u32(&mut image, size * 2);
    push_u16(&mut image, 1);
    push_u16(&mut image, 32);
    push_u32(&mut image, 0);
    push_u32(&mut image, bitmap_bytes + mask_bytes);
    image.extend_from_slice(&[0; 16]);

    for y in (0..size).rev() {
        for x in 0..size {
            let offset = ((y * size + x) * 4) as usize;
            image.extend_from_slice(&[
                rgba[offset + 2],
                rgba[offset + 1],
                rgba[offset],
                rgba[offset + 3],
            ]);
        }
    }

    for y in (0..size).rev() {
        let mut row = vec![0u8; mask_stride as usize];
        for x in 0..size {
            let alpha = rgba[((y * size + x) * 4 + 3) as usize];
            if alpha < 128 {
                row[(x / 8) as usize] |= 1 << (7 - x % 8);
            }
        }
        image.extend_from_slice(&row);
    }

    image
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::windows_icon;

    #[test]
    fn generated_ico_contains_valid_image_ranges() {
        let icon = windows_icon();
        assert_eq!(&icon[..6], &[0, 0, 1, 0, 7, 0]);

        for index in 0..7usize {
            let entry = 6 + index * 16;
            let length = u32::from_le_bytes(icon[entry + 8..entry + 12].try_into().unwrap());
            let offset = u32::from_le_bytes(icon[entry + 12..entry + 16].try_into().unwrap());
            assert!(length > 40);
            assert!(offset as usize + length as usize <= icon.len());
            assert_eq!(&icon[offset as usize..offset as usize + 4], &[40, 0, 0, 0]);
        }
    }
}
