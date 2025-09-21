use std::ffi::OsString;

use shared_files::core_header;

#[unsafe(no_mangle)]
extern "system" fn module_startup(core: &core_header::CoreH) {
    if core.args[1] == OsString::from("c") {
        let uncompressed_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let compressed_data = compress_v1(&uncompressed_data);
        let decompressed_data = decompress_v1(&compressed_data);
        println!(
            "Compressed data: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data, decompressed_data, uncompressed_data
        );
        let uncompressed_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let compressed_data = compress_v2(&uncompressed_data);
        let decompressed_data = decompress_v2(&compressed_data);
        println!(
            "Compressed data: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data, decompressed_data, uncompressed_data
        );

        let file_path = OsString::from("../../test.txt");
        let uncompressed_data = std::fs::read(file_path.clone()).unwrap();
        let compressed_data = compress_v1(&uncompressed_data);
        let decompressed_data = decompress_v1(&compressed_data);
        println!(
            "out legth: {:?} \n in length: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data.len(),
            uncompressed_data.len(),
            decompressed_data.len(),
            uncompressed_data.len()
        );
        let file_path = OsString::from("../../test.bmp");
        let uncompressed_data = std::fs::read(file_path.clone()).unwrap();
        let compressed_data = compress_v1(&uncompressed_data);
        let decompressed_data = decompress_v1(&compressed_data);
        println!(
            "out legth: {:?} \n in length: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data.len(),
            uncompressed_data.len(),
            decompressed_data.len(),
            uncompressed_data.len()
        );

        let file_path = OsString::from("../../test.pdf");
        let uncompressed_data = std::fs::read(file_path.clone()).unwrap();
        let compressed_data = compress_v1(&uncompressed_data);
        let decompressed_data = decompress_v1(&compressed_data);
        println!(
            "out legth: {:?} \n in length: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data.len(),
            uncompressed_data.len(),
            decompressed_data.len(),
            uncompressed_data.len()
        );

        let file_path = OsString::from("../../test.bmp");
        let uncompressed_data = std::fs::read(file_path).unwrap();
        let compressed_data = compress_v1(&uncompressed_data);
        let decompressed_data = decompress_v1(&compressed_data);
        println!(
            "out legth: {:?} \n in length: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data.len(),
            uncompressed_data.len(),
            decompressed_data.len(),
            uncompressed_data.len()
        );

        let file_path = OsString::from("../../test.txt");
        let uncompressed_data = std::fs::read(file_path.clone()).unwrap();
        let compressed_data = compress_v2(&uncompressed_data);
        let decompressed_data = decompress_v2(&compressed_data);
        println!(
            "out legth: {:?} \n in length: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data.len(),
            uncompressed_data.len(),
            decompressed_data.len(),
            uncompressed_data.len()
        );

        let file_path = OsString::from("../../test.bmp");
        let uncompressed_data = std::fs::read(file_path).unwrap();
        let compressed_data = compress_v2(&uncompressed_data);
        let decompressed_data = decompress_v2(&compressed_data);
        println!(
            "out legth: {:?} \n in length: {:?} \n Decompressed data: {:?} \n Uncompressed data: {:?} \n",
            compressed_data.len(),
            uncompressed_data.len(),
            decompressed_data.len(),
            uncompressed_data.len()
        );
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
    let mut compressed_data: Vec<u8> = Vec::new();
    let mut count: u8 = 0;
    for current_byte in uncompressed_data.iter().enumerate() {
        count += 1;
        if uncompressed_data.len() > current_byte.0 + 1
            && *current_byte.1 != uncompressed_data[current_byte.0 + 1]
            || count == 255
        {
            if count > 3 {
                compressed_data.push(u8::MIN);
                compressed_data.push(count);
                compressed_data.push(*current_byte.1);
            } else {
                compressed_data.push(*current_byte.1);
            }
            count = 0;
        }
    }
    if count > 3 {
        compressed_data.push(u8::MIN);
        compressed_data.push(count);
        compressed_data.push(uncompressed_data[uncompressed_data.len() - 1]);
    } else {
        compressed_data.push(uncompressed_data[uncompressed_data.len() - 1]);
    }
    return compressed_data;
}

fn decompress_v2(compressed_data: &Vec<u8>) -> Vec<u8> {
    let mut uncompressed_data: Vec<u8> = Vec::new();

    for i in (0..compressed_data.len()).step_by(1) {
        if compressed_data[i] == u8::MIN && compressed_data.len() > i + 2 {
            let count = compressed_data[i + 1];
            let byte = compressed_data[i + 2];
            for _j in 0..count {
                uncompressed_data.push(byte);
            }
        } else {
            uncompressed_data.push(compressed_data[i]);
        }
    }
    return uncompressed_data;
}
