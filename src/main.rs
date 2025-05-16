use image::{ImageBuffer, Rgba, GenericImageView, AnimationDecoder, DynamicImage};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_multipart::Multipart;
use actix_files as fs;
use serde::Serialize;
use std::path::Path;
use futures::{StreamExt, TryStreamExt};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};


#[derive(Serialize)]
struct ProcessResponse {
    message: String,
    combine_data: String,
    mask_data: String,
}

// Apply slit transparency effect to a single image
fn apply_slit_transparency(
    img: &DynamicImage,
    slit_width: u32,
    slit_spacing: u32,
    frame_number: u32,
) -> DynamicImage {
    let (width, height) = img.dimensions();
    println!("Processing image with dimensions: {}x{}", width, height);

    // Calculate the offset based on frame number
    let offset = slit_width * frame_number;

    // Create a new image buffer with RGBA pixels
    let output_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
        let pixel = img.get_pixel(x, y);
        let adjusted_x = (x + width - offset) % width;
        if adjusted_x % (slit_spacing + slit_width) >= slit_width {
            // Make the pixel fully transparent in slit areas
            Rgba([pixel[0], pixel[1], pixel[2], 0])
        } else {
            // Keep the original pixel
            pixel
        }
    });
    return DynamicImage::ImageRgba8(output_img);
}

// Process a GIF file, extracting each frame and applying the effect
fn process_gif_file(
    buffer: Vec<u8>,
    slit_width: u32,
    slit_spacing: u32,
    frame_count: u32,
) -> Vec<DynamicImage> {
    // Decode GIF frames directly from memory
    let frames = match image::codecs::gif::GifDecoder::new(std::io::Cursor::new(buffer)) {
        Ok(decoder) => decoder.into_frames().collect_frames().expect("Failed to collect frames"),
        Err(e) => {
            println!("Error creating GIF decoder: {:?}", e);
            return vec![];
        }
    };
    
    println!("Found {} frames in the GIF", frames.len());
    
    // Process each frame
    let mut processed_frames = vec![];
    let frames_per_file = frames.len() as u32 / frame_count;
    let mut count = 0;
    for (i, frame) in frames.into_iter().enumerate() {
        if i as u32 % frames_per_file == 0 {
            let frame_img = DynamicImage::ImageRgba8(frame.into_buffer());
            processed_frames.push(apply_slit_transparency(&frame_img, slit_width, slit_spacing, count));
            count += 1;
        }
    }

    return processed_frames; 
}

// Combine multiple PNG files, skipping transparent pixels
fn combine_png_files_skip_transparent(
    frames: Vec<DynamicImage>
) -> Vec<u8> {
    // Load the first image to get dimensions
    let first_img = frames.first().expect("データの読み込みに失敗しました。");
    let (width, height) = first_img.dimensions();
    
    // Create a new image buffer with RGBA pixels
    let mut combined_img = ImageBuffer::new(width, height);
    
    // Process each file
    for img in frames.iter() {
        // Copy non-transparent pixels
        for y in 0..height {
            for x in 0..width {
                let pixel = img.get_pixel(x, y);
                // Only copy non-transparent pixels
                if pixel[3] > 0 {
                    combined_img.put_pixel(x, y, pixel);
                }
            }
        }
    }
    
    // PNGとしてエンコード
    let mut png_data = Vec::new();
    let dynamic_img = DynamicImage::ImageRgba8(combined_img);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut png_data), image::ImageFormat::Png)
        .expect("PNGエンコードに失敗しました");
    
    return png_data;
}

// Create a striped mask image
fn create_stripe_mask(
    width: u32,
    height: u32,
    stripe_width: u32,
    stripe_spacing: u32
) -> Vec<u8> {
    // Create a new image buffer with RGBA pixels
    let mut mask_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    
    for y in 0..height {
        for x in 0..width {
            if x % (stripe_spacing + stripe_width) < stripe_width {
                // Black stripe (fully transparent)
                mask_image.put_pixel(x, y, Rgba([0,0,0,0]));
            } else {
                // White stripe (fully opaque)
                mask_image.put_pixel(x, y, Rgba([0,0,0,255]));
            }
        }
    }
    
    // PNGとしてエンコード
    let mut png_data = Vec::new();
    let dynamic_img = DynamicImage::ImageRgba8(mask_image);
    dynamic_img.write_to(&mut std::io::Cursor::new(&mut png_data), image::ImageFormat::Png)
        .expect("PNGエンコードに失敗しました");
    
    return png_data;
}

async fn process_image(mut payload: Multipart) -> impl Responder {
    let mut slit_width = 5;
    let mut frame_count = 8;
    let mut gif_buffer = Vec::new();

    // マルチパートデータの処理
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition.get_name().unwrap_or("");

        match field_name {
            "file" => {
                // メモリ上にGIFデータを保存
                while let Some(chunk) = field.next().await {
                    let data = chunk.unwrap();
                    gif_buffer.extend_from_slice(&data);
                }
            }
            "slit_width" => {
                let mut value = String::new();
                while let Some(chunk) = field.next().await {
                    let data = chunk.unwrap();
                    value.push_str(std::str::from_utf8(&data).unwrap());
                }
                if let Ok(width) = value.parse::<u32>() {
                    slit_width = width;
                }
            }
            "frame_count" => {
                let mut value = String::new();
                while let Some(chunk) = field.next().await {
                    let data = chunk.unwrap();
                    value.push_str(std::str::from_utf8(&data).unwrap());
                }
                if let Ok(count) = value.parse::<u32>() {
                    frame_count = count;
                }
            }
            _ => {}
        }
    }

    // ファイルがアップロードされていない場合
    if gif_buffer.is_empty() {
        return HttpResponse::BadRequest().body("ファイルがアップロードされていません");
    }

    // パラメータの設定
    let slit_spacing = frame_count * slit_width;

    // GIFファイルの処理
    let frames = process_gif_file(gif_buffer, slit_width, slit_spacing, frame_count);
    let first_img = frames.first().expect("データの読み込みに失敗しました。");
    let (width, height) = first_img.dimensions();

    // フレームの結合
    let combine_data = combine_png_files_skip_transparent(frames);
    
    let mask_data = create_stripe_mask(width, height, slit_width, slit_spacing);

    HttpResponse::Ok().json(ProcessResponse {
        message: "処理が完了しました".to_string(),
        combine_data: BASE64.encode(&combine_data),
        mask_data: BASE64.encode(&mask_data),
    })
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body(include_str!("../static/index.html"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("サーバーを起動しています...");
    
    if !Path::new("static").exists() {
        std::fs::create_dir("static")?;
    }
    
    HttpServer::new(|| {
        App::new()
            .service(fs::Files::new("/static", "static").index_file("index.html"))
            .route("/", web::get().to(index))
            .route("/process", web::post().to(process_image))
    })
    .bind("127.0.0.1:8081")?
    .run()
    .await
}