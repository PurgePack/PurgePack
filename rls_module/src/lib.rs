use std::{ffi::OsString, io::Write};

use shared_files::core_header;

const MAX_RUN_LENGTH: u8 = u8::MAX;
const ESCAPE_BYTE: u8 = u8::MIN;

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
/// Compresses a byte array using the most basic Run-Length Encoding algorithm.
///
/// This function is not optimized for data without repeated bytes, as it can
/// cause the compressed data size to be larger than the original.
///
/// # Arguments
///
/// * `uncompressed_data` - The byte array to compress.
///
/// # Returns
///
/// A `Vec<u8>` containing the compressed data.
///
/// # Edge Cases
///
/// * If the input byte array is empty, the function returns an empty vector.
/// * This algorithm has an upper limit of 255 consecutive identical bytes,
///   due to `count` being a `u8`. **If a run exceeds 255 bytes, it is split**
///   into multiple RLE pairs (e.g., a 268-byte run of `0x04` becomes
///   `[255, 4, 13, 4]`).
///
/// # Example
///
/// ```rust
/// let uncompressed_data = vec![1, 2, 3, 4, 5];
/// let compressed_data = compress_v1(&uncompressed_data);
/// assert_eq!(compressed_data, vec![1, 1, 1, 2, 1, 3, 1, 4, 1, 5]);
///
/// let uncompressed_data_repeated = vec![7, 7, 7, 8, 8, 9];
/// let compressed_data_repeated = compress_v1(&uncompressed_data_repeated);
/// assert_eq!(compressed_data_repeated, vec![3, 7, 2, 8, 1, 9]);
/// ```
fn compress_v1(uncompressed_data: &[u8]) -> Vec<u8> {
    if uncompressed_data.is_empty() {
        return Vec::new();
    }

    let mut compressed_data = Vec::with_capacity(uncompressed_data.len() * 2);
    let mut current_byte = uncompressed_data[0];
    let mut count: u8 = 1;

    for &byte in uncompressed_data.iter().skip(1) {
        if byte == current_byte && count < MAX_RUN_LENGTH {
            count += 1;
        } else {
            compressed_data.push(count);
            compressed_data.push(current_byte);
            current_byte = byte;
            count = 1;
        }
    }
    compressed_data.push(count);
    compressed_data.push(current_byte);

    compressed_data
}

/// Decompresses a byte array using the Run-Length Encoding algorithm.
///
/// This function takes a slice of compressed bytes and returns the decompressed
/// data as a new vector. It accurately pre-allocates the memory needed for the
/// output, ensuring optimal performance.
///
/// # Arguments
///
/// * `compressed_data` - A slice of bytes containing the RLE-encoded data.
///
/// # Returns
///
/// A `Result` which is either:
/// * `Ok(Vec<u8>)` - The successfully decompressed data.
/// * `Err(&'static str)` - An error message if the input is malformed.
///
/// # Edge Cases
///
/// * If the input byte array is empty, the function returns an empty vector.
/// * This function returns an `Err` if the length of `compressed_data` is odd,
///   as this indicates a corrupt or incomplete data stream.
///
/// # Example
///
/// ```rust
/// let compressed_data = vec![3, 65, 2, 66, 1, 67]; // 3 'A's, 2 'B's, 1 'C'
/// let decompressed_data = decompress_v1(&compressed_data).unwrap();
/// assert_eq!(decompressed_data, vec![65, 65, 65, 66, 66, 67]);
///
/// // Example of invalid input that returns an Err
/// let invalid_compressed_data = vec![1, 2, 3];
/// assert!(decompress_v1(&invalid_compressed_data).is_err());
/// ```
fn decompress_v1(compressed_data: &[u8]) -> Result<Vec<u8>, &'static str> {
    if compressed_data.is_empty() {
        return Ok(Vec::new());
    }
    if compressed_data.len() % 2 != 0 {
        return Err("Invalid compressed data length");
    }
    let estimated_size: usize = compressed_data
        .chunks_exact(2)
        .map(|chunk| chunk[0] as usize)
        .sum();
    let mut uncompressed_data = Vec::with_capacity(estimated_size);

    for chunk in compressed_data.chunks_exact(2) {
        let count = chunk[0];
        let byte = chunk[1];
        uncompressed_data.extend(std::iter::repeat(byte).take(count as usize));
    }
    return Ok(uncompressed_data);
}
/// Compresses a byte array using an improved Run-Length Encoding algorithm.
///
/// This version avoids expanding the data for short runs (1, 2, or 3 bytes)
/// by directly inserting the bytes, using a special `ESCAPE_BYTE` to denote
/// runs of 4 or more, or runs of the escape byte itself.
///
/// # Arguments
///
/// * `uncompressed_data` - The byte array to compress.
///
/// # Returns
///
/// A `Vec<u8>` containing the compressed data.
///
/// # Edge Cases
///
/// * If the input byte array is empty, the function returns an empty vector.
/// * The algorithm has an upper limit of 255 consecutive identical bytes,
///   due to `count` being a `u8`. **If a run exceeds 255 bytes, it is split**
///   into multiple RLE triplets (e.g., a 268-byte run of `0x04` becomes
///   `[ESCAPE, 255, 4, ESCAPE, 13, 4]`).
///
/// # Example
///
/// ```rust
/// // Assuming ESCAPE_BYTE = u8::MIN (0) and the RLE threshold is 4.
///
/// // Case 1: Short runs (1, 2, 3) are written literally.
/// let uncompressed_data_short = vec![1, 2, 3, 4, 4, 5, 5, 5];
/// // Expected: 1, 2, 3, 4, 4, 5, 5, 5 (all literal)
/// let compressed_data_short = compress_v2(&uncompressed_data_short);
/// assert_eq!(compressed_data_short, vec![1, 2, 3, 4, 4, 5, 5, 5]);
///
/// // Case 2: Long run (5x 6s) is encoded as a triplet: [ESCAPE, count, byte]
/// let uncompressed_data_long = vec![1, 6, 6, 6, 6, 6, 7];
/// // Expected: 1 (literal), u8::MIN, 5, 6, 7 (literal)
/// let compressed_data_long = compress_v2(&uncompressed_data_long);
/// assert_eq!(compressed_data_long, vec![1, u8::MIN, 5, 6, 7]);
///
/// // Case 3: A run of the ESCAPE_BYTE itself (must be encoded)
/// let uncompressed_data_escape = vec![u8::MIN, u8::MIN];
/// // Expected: u8::MIN, 2, u8::MIN
/// let compressed_data_escape = compress_v2(&uncompressed_data_escape);
/// assert_eq!(compressed_data_escape, vec![u8::MIN, 2, u8::MIN]);
/// ```
fn compress_v2(uncompressed_data: &[u8]) -> Vec<u8> {
    if uncompressed_data.is_empty() {
        return Vec::new();
    }

    let mut compressed_data: Vec<u8> = Vec::with_capacity(uncompressed_data.len());
    let mut count: u8 = 1;
    let mut current_byte = uncompressed_data[0];
    for &byte in uncompressed_data.iter().skip(1) {
        if byte == current_byte && count < MAX_RUN_LENGTH {
            count += 1;
        } else {
            push_to_compressed_data(&mut compressed_data, count, current_byte);
            current_byte = byte;
            count = 1;
        }
    }
    push_to_compressed_data(&mut compressed_data, count, current_byte);

    return compressed_data;
}
/// Decompresses a byte array encoded with the improved RLE algorithm (v2).
///
/// This function processes the compressed stream, differentiating between
/// literal bytes and RLE triplets (Escape Byte + Count + Data Byte).
///
/// # Arguments
///
/// * `compressed_data` - A slice of bytes containing the RLE-encoded data.
///
/// # Returns
///
/// A `Vec<u8>` containing the successfully decompressed data.
///
/// # Edge Cases
///
/// * If the input byte array is empty, the function returns an empty vector.
/// * If the `ESCAPE_BYTE` is encountered but the remaining data is too short
///   to form a valid triplet (i.e., less than 3 bytes remain), the remaining
///   bytes are treated as literal data. This handles potential corruption.
///
/// # Example
///
/// /// # Example (Added Example)
///
/// ```rust
/// // Assuming ESCAPE_BYTE = u8::MIN (0)
/// // Compressed: 1 (literal), 0, 5, 6 (5x 6s), 7 (literal)
/// let compressed_data = vec![1, u8::MIN, 5, 6, 7];
/// let decompressed_data = decompress_v2(&compressed_data);
/// assert_eq!(decompressed_data, vec![1, 6, 6, 6, 6, 6, 7]);
///
/// // Compressed: 0, 2, 0 (2x 0s), 99 (literal)
/// let compressed_data_escape = vec![u8::MIN, 2, u8::MIN, 99];
/// let decompressed_data_escape = decompress_v2(&compressed_data_escape);
/// assert_eq!(decompressed_data_escape, vec![0, 0, 99]);
/// ```
fn decompress_v2(compressed_data: &[u8]) -> Result<Vec<u8>, &'static str> {
    if compressed_data.is_empty() {
        return Ok(Vec::new());
    }
    let mut uncompressed_data: Vec<u8> = Vec::with_capacity(compressed_data.len());
    let mut uncompressed_data_index = 0;
    while uncompressed_data_index < compressed_data.len() {
        if compressed_data[uncompressed_data_index] == ESCAPE_BYTE {
            if uncompressed_data_index + 2 > compressed_data.len() {
                return Err("Invalid RLE triplet in compressed data.");
            }
            let count = compressed_data[uncompressed_data_index + 1];
            let byte = compressed_data[uncompressed_data_index + 2];
            uncompressed_data.extend(std::iter::repeat(byte).take(count as usize));
            uncompressed_data_index += 3;
        } else {
            uncompressed_data.push(compressed_data[uncompressed_data_index]);
            uncompressed_data_index += 1;
        }
    }
    return Ok(uncompressed_data);
}
/// Helper function for `compress_v2` to decide how to encode a run of bytes.
///
/// It implements a hybrid RLE strategy:
/// * Short runs (1, 2, or 3 bytes) are written literally to save space.
/// * Long runs (4+ bytes) or runs of the `ESCAPE_BYTE` are written as an
///   RLE triplet: `[ESCAPE_BYTE, count, byte]`.
///
/// # Arguments
///
/// * `compressed_data` - A mutable reference to the vector storing the
///   compressed data. The bytes will be appended here.
/// * `count` - The number of times the byte is repeated (1 to 255).
/// * `current_byte` - The byte that is repeated.
///
/// # Returns
///
/// This function does not return a value; it modifies `compressed_data` in place.
///
/// # Edge Cases
///
/// * If `current_byte` is equal to `ESCAPE_BYTE`, it is always encoded as a
///   triplet, regardless of the `count`, to ensure the `ESCAPE_BYTE` is
///   not mistaken for a literal run marker.
fn push_to_compressed_data(compressed_data: &mut Vec<u8>, count: u8, current_byte: u8) {
    if count > 3 || current_byte == ESCAPE_BYTE {
        compressed_data.push(ESCAPE_BYTE);
        compressed_data.push(count);
        compressed_data.push(current_byte);
    } else {
        compressed_data.extend(std::iter::repeat(current_byte).take(count as usize));
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
    let decompressed_data;
    match version.to_string_lossy().as_ref() {
        "v1" => decompressed_data = decompress_v1(&uncompressed_data),
        "v2" => decompressed_data = decompress_v2(&uncompressed_data),
        _ => decompressed_data = decompress_v1(&uncompressed_data),
    }

    match decompressed_data {
        Ok(data) => {
            decompressed_data = data;
        }
        Err(e) => {
            println!("{}", e);
            return;
        }
    }
    if deploy == OsString::from("preview") {
        println!("\n--- Compression Statistics ---");
        println!("  Original Size:    {} bytes", uncompressed_data.len());
        println!("  decompressed Size:  {} bytes", decompressed_data.len());
        println!(
            "  Compression Ratio: {:.2}",
            decompressed_data.len() as f32 / uncompressed_data.len() as f32
        );
    }
    let mut decompressed_data_file = std::fs::File::create(output_file_path.clone()).unwrap();
    if output_file_path
        .to_string_lossy()
        .as_ref()
        .ends_with(".txt")
    {
        String::from_utf8(decompressed_data.clone()).unwrap();
    }
    decompressed_data_file
        .write_all(&decompressed_data)
        .unwrap();
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
