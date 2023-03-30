use std::{fs, path::PathBuf};

use base64::{engine::general_purpose, Engine};

pub fn read_favicon(path: String) -> Option<String> {
    let favicon_file = PathBuf::from(path);

    if !favicon_file.exists() {
        println!(
            "doesnt exist {}",
            favicon_file.as_os_str().to_str().unwrap()
        );
        return None;
    }

    let favicon = fs::read(favicon_file).unwrap();
    let favicon_meta = image_meta::load_from_buf(&favicon).unwrap();

    if favicon_meta.dimensions.width != 64 || favicon_meta.dimensions.height != 64 {
        println!("Favicon isn't 64x64");
        return None;
    };

    let mut buf = "data:image/png;base64,".to_string();
    general_purpose::STANDARD.encode_string(favicon, &mut buf);

    Some(buf)
}
