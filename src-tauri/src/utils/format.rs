//! 格式处理

use image::codecs::jpeg::JpegEncoder;
use base64::Engine;

// 辅助函数：生成图片缩略图并压缩至 ≤10KB
pub fn generate_image_thumbnail(image_data: &[u8], max_size_kb: usize) -> Result<String, Box<dyn std::error::Error>> {
    // 加载原始图片
    let img = image::load_from_memory(image_data)?;

    // 计算缩略图尺寸（最大边长 200 像素，保持宽高比）
    let max_dim = 200;
    let (width, height) = (img.width(), img.height());
    let (new_width, new_height) = if width > height {
        (max_dim, (height * max_dim / width).max(1))
    } else {
        ((width * max_dim / height).max(1), max_dim)
    };
    let thumbnail = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);

    // 尝试不同 JPEG 质量，直到文件大小 ≤ max_size_kb 或质量低于 50
    let mut quality = 85;
    let mut thumbnail_bytes = Vec::new();
    loop {
        thumbnail_bytes.clear();
        let mut encoder = JpegEncoder::new_with_quality(&mut thumbnail_bytes, quality);
        encoder.encode_image(&thumbnail)?;
        let size_kb = thumbnail_bytes.len() / 1024;
        if size_kb <= max_size_kb || quality <= 50 {
            break;
        }
        quality -= 5;
    }

    // 将字节转换为 Base64 字符串
    Ok(base64::engine::general_purpose::STANDARD.encode(&thumbnail_bytes))
}


pub fn normalize_file_uri(path: &str) -> &str {
    if let Some(rest) = path.strip_prefix("file:///") {
        rest
    } else if let Some(rest) = path.strip_prefix("file://") {
        rest
    } else {
        path
    }
}
