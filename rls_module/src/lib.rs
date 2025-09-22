use std::{ffi::OsString, io::Write};

use shared_files::core_header;

#[unsafe(no_mangle)]
extern "system" fn module_startup(core: &core_header::CoreH) {
    if core.args[1] == OsString::from("c") {
        compress_from_file(
            core.args[2].clone(),
            core.args[3].clone(),
            core.args[4].clone(),
            core.args[5].clone(),
        );
    } else if core.args[1] == OsString::from("d") {
        decompress_from_file(
            core.args[2].clone(),
            core.args[3].clone(),
            core.args[4].clone(),
            core.args[5].clone(),
        );
    } else {
        print!("format should be <c|d> <in_file> <out_file> <version>");
        return;
    }
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &mut core_header::CoreH, _exiting: bool) {
    println!("Goodbye world!");
}

fn compress_v1(uncompressed_data: &Vec<u8>) -> Vec<u8> {
    if uncompressed_data.is_empty() {
        return Vec::new();
    }

    let mut compressed_data: Vec<u8> = Vec::new();
    let mut current_byte = uncompressed_data[0];
    let mut count: u8 = 1;

    for i in 1..uncompressed_data.len() {
        if uncompressed_data[i] == current_byte && count < 255 {
            count += 1;
        } else {
            compressed_data.push(count);
            compressed_data.push(current_byte);
            current_byte = uncompressed_data[i];
            count = 1;
        }
    }
    compressed_data.push(count);
    compressed_data.push(current_byte);

    compressed_data
}

fn decompress_v1(compressed_data: &Vec<u8>) -> Vec<u8> {
    let mut uncompressed_data: Vec<u8> = Vec::new();

    for chunk in compressed_data.chunks_exact(2) {
        let count = chunk[0];
        let byte = chunk[1];
        for _ in 0..count {
            uncompressed_data.push(byte);
        }
    }
    uncompressed_data
}

fn compress_v2(uncompressed_data: &Vec<u8>) -> Vec<u8> {
    if uncompressed_data.is_empty() {
        return Vec::new();
    }

    let mut compressed_data: Vec<u8> = Vec::new();
    let mut count: u8 = 1;
    let mut current_byte = uncompressed_data[0];
    for i in (1..uncompressed_data.len()).step_by(1) {
        if uncompressed_data[i] == current_byte && count < 255 {
            count += 1;
        } else {
            push_to_compressed_data(&mut compressed_data, count, current_byte);
            current_byte = uncompressed_data[i];
            count = 1;
        }
    }
    push_to_compressed_data(&mut compressed_data, count, current_byte);

    return compressed_data;
}

fn decompress_v2(compressed_data: &Vec<u8>) -> Vec<u8> {
    if compressed_data.is_empty() {
        return Vec::new();
    }
    let mut uncompressed_data: Vec<u8> = Vec::new();
    let mut uncompressed_data_index = 0;
    while uncompressed_data_index < compressed_data.len() {
        if compressed_data[uncompressed_data_index] == u8::MIN
            && compressed_data.len() > uncompressed_data_index + 2
        {
            let count = compressed_data[uncompressed_data_index + 1];
            let byte = compressed_data[uncompressed_data_index + 2];
            for _j in 0..count {
                uncompressed_data.push(byte);
            }
            uncompressed_data_index += 3;
        } else {
            uncompressed_data.push(compressed_data[uncompressed_data_index]);
            uncompressed_data_index += 1;
        }
    }
    return uncompressed_data;
}

fn push_to_compressed_data(compressed_data: &mut Vec<u8>, count: u8, current_byte: u8) {
    if count > 3 || current_byte == u8::MIN {
        compressed_data.push(u8::MIN);
        compressed_data.push(count);
        compressed_data.push(current_byte);
    } else {
        for _ in 0..count {
            compressed_data.push(current_byte);
        }
    }
}

fn compress_from_file(
    deploy: OsString,
    input_file_path: OsString,
    output_file_path: OsString,
    version: OsString,
) {
    let uncompressed_data = std::fs::read(input_file_path.clone()).unwrap();
    let compressed_data;
    match version.to_string_lossy().as_ref() {
        "v1" => compressed_data = compress_v1(&uncompressed_data),
        "v2" => compressed_data = compress_v2(&uncompressed_data),
        "auto" => match auto_choice(&uncompressed_data) {
            "v1" => compressed_data = compress_v1(&uncompressed_data),
            "v2" => compressed_data = compress_v2(&uncompressed_data),
            _ => compressed_data = compress_v1(&uncompressed_data),
        },
        _ => compressed_data = compress_v1(&uncompressed_data),
    }
    if deploy == OsString::from("preview") {
        println!("\n--- Compression Statistics ---");
        println!("  Original Size:    {} bytes", uncompressed_data.len());
        println!("  Compressed Size:  {} bytes", compressed_data.len());
        println!(
            "  Compression Ratio: {:.2}",
            compressed_data.len() as f32 / uncompressed_data.len() as f32
        );
    }

    let mut compressed_data_file = std::fs::File::create(output_file_path).unwrap();
    compressed_data_file.write_all(&compressed_data).unwrap();
}

fn decompress_from_file(
    deploy: OsString,
    file_path: OsString,
    output_file_path: OsString,
    version: OsString,
) {
    if !file_path.to_string_lossy().as_ref().ends_with(".purgepack") {
        println!("Not a purgepack compressed file");
        return;
    }
    let uncompressed_data = std::fs::read(file_path).unwrap();
    let compressed_data;
    match version.to_string_lossy().as_ref() {
        "v1" => compressed_data = decompress_v1(&uncompressed_data),
        "v2" => compressed_data = decompress_v2(&uncompressed_data),
        _ => compressed_data = decompress_v1(&uncompressed_data),
    }
    if deploy == OsString::from("preview") {
        println!("\n--- Compression Statistics ---");
        println!("  Original Size:    {} bytes", uncompressed_data.len());
        println!("  decompressed Size:  {} bytes", compressed_data.len());
        println!(
            "  Compression Ratio: {:.2}",
            compressed_data.len() as f32 / uncompressed_data.len() as f32
        );
    }
    let mut decompressed_data_file = std::fs::File::create(output_file_path.clone()).unwrap();
    if output_file_path
        .to_string_lossy()
        .as_ref()
        .ends_with(".txt")
    {
        String::from_utf8(compressed_data.clone()).unwrap();
    }
    decompressed_data_file.write_all(&compressed_data).unwrap();
}

fn auto_choice(uncompressed_data: &Vec<u8>) -> &'static str {
    let compressed_data_v1 = compress_v1(uncompressed_data);
    let compressed_data_v2 = compress_v2(uncompressed_data);

    if compressed_data_v1.len() >= compressed_data_v2.len() {
        return "v2";
    } else {
        return "v1";
    }
}
