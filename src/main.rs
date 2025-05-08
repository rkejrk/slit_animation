use image::{ImageBuffer, Rgba, GenericImageView, io::Reader as ImageReader, AnimationDecoder, DynamicImage};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_multipart::Multipart;
use actix_files as fs;
use serde::{Deserialize, Serialize};
use std::path::Path;
use futures::{StreamExt, TryStreamExt};

#[derive(Deserialize)]
struct SlitParams {
    slit_width: u32,
    frame_count: u32,
}

#[derive(Serialize)]
struct ProcessResponse {
    message: String,
    output_path: String,
    mask_path: String,
}

// Apply slit transparency effect to a single image
fn apply_slit_transparency(
    img: &DynamicImage,
    output_image_path: &str,
    slit_width: u32,
    slit_spacing: u32,
    frame_number: u32,
) {
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

    // Save the output image
    match output_img.save(output_image_path) {
        Ok(_) => println!("Output saved to {}", output_image_path),
        Err(e) => println!("Error saving output image: {:?}", e),
    }
}

// Process a GIF file, extracting each frame and applying the effect
fn process_gif_file(
    input_path: &str,
    output_dir: &str,
    slit_width: u32,
    slit_spacing: u32,
    frame_count: u32,
) {
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // Open the GIF file
    let file = match File::open(input_path) {
        Ok(file) => file,
        Err(e) => {
            println!("Error opening file: {:?}", e);
            return;
        }
    };
    
    let reader = BufReader::new(file);
    
    // Use with_format to explicitly specify GIF format
    let decoder = match ImageReader::new(reader)
        .with_guessed_format()
        .unwrap()
        .decode() {
        Ok(data) => data,
        Err(e) => {
            println!("Error decoding GIF: {:?}", e);
            return;
        }
    };

    // For GIF files, we need to re-open to get the frames
    let file = match File::open(input_path) {
        Ok(file) => file,
        Err(e) => {
            println!("Error reopening file: {:?}", e);
            return;
        }
    };
    
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    if let Err(e) = reader.read_to_end(&mut buffer) {
        println!("Error reading file: {:?}", e);
        return;
    }
    
    // Decode GIF frames
    let frames = match image::codecs::gif::GifDecoder::new(std::io::Cursor::new(buffer)) {
        Ok(decoder) => decoder.into_frames().collect_frames().expect("Failed to collect frames"),
        Err(e) => {
            println!("Error creating GIF decoder: {:?}", e);
            return;
        }
    };
    
    println!("Found {} frames in the GIF", frames.len());
    
    // Process each frame
    let frames_per_file = frames.len() as u32 / frame_count;
    let mut count = 0;
    for (i, frame) in frames.into_iter().enumerate() {
        if i as u32 % frames_per_file == 0 {
            let frame_img = DynamicImage::ImageRgba8(frame.into_buffer());
            let output_path = format!("{}/frame_{:03}.png", output_dir, i);
            apply_slit_transparency(&frame_img, &output_path, slit_width, slit_spacing, count);
            count += 1;
        }
    }
    
    println!("All frames processed and saved to {}", output_dir);
}

// Combine multiple PNG files, skipping transparent pixels
fn combine_png_files_skip_transparent(
    input_dir: &str,
    output_path: &str,
) {
    // Get all PNG files in the directory
    let mut files: Vec<_> = std::fs::read_dir(input_dir)
        .expect("Failed to read directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "png" {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    
    // Sort files by name
    files.sort();
    
    if files.is_empty() {
        println!("No PNG files found in directory");
        return;
    }

    // Load the first image to get dimensions
    let first_img = image::open(&files[0]).expect("Failed to open first image");
    let (width, height) = first_img.dimensions();
    
    // Create a new image buffer with RGBA pixels
    let mut combined_img = ImageBuffer::new(width, height);
    
    // Process each file
    for file_path in files.iter() {
        let img = image::open(file_path).expect("Failed to open image");
        
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
    
    // Save the combined image
    match combined_img.save(output_path) {
        Ok(_) => println!("Combined image saved to {}", output_path),
        Err(e) => println!("Error saving combined image: {:?}", e),
    }
}

// Create a striped mask image
fn create_stripe_mask(
    combined_output: &str,
    stripe_width: u32,
    stripe_spacing: u32,
    output_path: &str,
) { 
    let img = image::open(combined_output).expect("Failed to open combined image");
    let (width, height) = img.dimensions();
    // Create a new image buffer with RGBA pixels
    let mut mask_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    
    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if x % (stripe_spacing + stripe_width) < stripe_width {
                // Black stripe (fully transparent)
                mask_image.put_pixel(x, y, Rgba([0,0,0,0]));
            } else {
                // White stripe (fully opaque)
                mask_image.put_pixel(x, y, Rgba([0,0,0,255]));
            }
        }
    }
    
    // Save the mask image
    match mask_image.save(output_path) {
        Ok(_) => println!("Stripe mask saved to {}", output_path),
        Err(e) => println!("Error saving stripe mask: {:?}", e),
    }
}

async fn process_image(mut payload: Multipart) -> impl Responder {
    let mut slit_width = 5;
    let mut frame_count = 8;
    let mut temp_file_path = None;

    // マルチパートデータの処理
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();
        let field_name = content_disposition.get_name().unwrap_or("");

        match field_name {
            "file" => {
                // 一時ファイルの作成
                let temp_path = format!("static/temp_{}.gif", chrono::Utc::now().timestamp());
                let mut file = File::create(&temp_path).unwrap();
                
                // ファイルの保存
                while let Some(chunk) = field.next().await {
                    let data = chunk.unwrap();
                    file.write_all(&data).unwrap();
                }
                
                temp_file_path = Some(temp_path);
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
    let input_path = match temp_file_path {
        Some(path) => path,
        None => return HttpResponse::BadRequest().body("ファイルがアップロードされていません"),
    };

    let output_dir = format!("static/output_{}", chrono::Utc::now().timestamp());
    let combined_output = format!("{}/combined.png", output_dir);
    let mask_output = format!("{}/mask.png", output_dir);

    // 出力ディレクトリの作成
    std::fs::create_dir_all(&output_dir).unwrap();

    // パラメータの設定
    let slit_spacing = frame_count * slit_width;

    // GIFファイルの処理
    process_gif_file(&input_path, &output_dir, slit_width, slit_spacing, frame_count);
    
    // フレームの結合
    combine_png_files_skip_transparent(&output_dir, &combined_output);
    
    // マスクの作成
    create_stripe_mask(&combined_output, slit_width, slit_spacing, &mask_output);

    // 一時ファイルの削除
    let _ = std::fs::remove_file(input_path);

    HttpResponse::Ok().json(ProcessResponse {
        message: "処理が完了しました".to_string(),
        output_path: combined_output,
        mask_path: mask_output,
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