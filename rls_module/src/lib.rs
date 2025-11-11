use crate::cli_parse::Version;
use rand::Rng;
use shared_files::core_header;
use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    path::PathBuf,
};
mod cli_parse;

const MAX_RUN_LENGTH: u8 = u8::MAX;
const ESCAPE_BYTE: u8 = u8::MIN;
const CHUNK_SIZE_BYTES: usize = 1024;
const NUM_CHUNKS: usize = 5;

#[unsafe(no_mangle)]
extern "system" fn module_startup(_core: &core_header::CoreH) {
    match cli_parse::parse_args() {
        Ok(args) => {
            println!("Valid argument configuration loaded: {:?}", args);
            match args.command {
                cli_parse::Commands::Compress {
                    input_file,
                    output_file,
                } => {
                    println!(
                        "Compression: Input: {}, Output: {}, Version: {}, Statistics: {}",
                        input_file.display(),
                        output_file.display(),
                        args.rle_version,
                        args.stats,
                    );

                    compress_from_file(input_file, output_file, args.rle_version, args.stats);
                }
                cli_parse::Commands::Decompress {
                    input_file,
                    output_file,
                } => {
                    println!(
                        "Decompression: Input: {}, Output: {}, Version: {}, Statistics: {}",
                        input_file.display(),
                        output_file.display(),
                        args.rle_version,
                        args.stats,
                    );

                    decompress_from_file(input_file, output_file, args.rle_version, args.stats);
                }
            }
        }
        Err(cli_parse::CliError::ClapError(e)) => {
            println!("Error during argument parsing:");
            eprintln!("{}", e);
        }
        Err(e) => {
            println!("Error during argument validation:");
            match e {
                cli_parse::CliError::InputFileNotFound(path) => {
                    println!("Error: Input file does not exist: {}", path.display());
                }
                cli_parse::CliError::InputNotFile(path) => {
                    println!("Error: Input path is not a file: {}", path.display());
                }
                cli_parse::CliError::OutputParentDirNotFound(path) => {
                    println!(
                        "Error: The output directory does not exist: {}",
                        path.display()
                    );
                    println!("Please ensure the directory is created: {}", path.display());
                }
                cli_parse::CliError::OutputParentNotDir(path) => {
                    println!(
                        "Error: The parent path of the output file is not a directory: {}",
                        path.display()
                    );
                }
                _ => {
                    eprintln!("Unhandled argument error: {:?}", e);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &core_header::CoreH) {
    println!("RLS Module shutdown!");
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
    input_file_path: PathBuf,
    output_file_path: PathBuf,
    version: cli_parse::Version,
    is_stats_enabled: bool,
) {
    let uncompressed_data = match std::fs::read(input_file_path.clone()) {
        Ok(data) => data,
        Err(e) => {
            eprintln!(
                "Error reading input file {}: {}",
                input_file_path.to_string_lossy(),
                e
            );
            return;
        }
    };

    let compressed_data: Vec<u8>;

    let uncompressed_len = uncompressed_data.len();

    println!("{:?}", version);
    // Determine which version to use and execute compression
    match version {
        cli_parse::Version::One => compressed_data = compress_v1(&uncompressed_data),
        cli_parse::Version::Two => compressed_data = compress_v2(&uncompressed_data),
        cli_parse::Version::Auto => {
            let random_chunks = read_multiple_random_chunks(&input_file_path).unwrap();
            let choice = auto_choice_from_chunks(&random_chunks);
            match choice {
                cli_parse::Version::One => compressed_data = compress_v1(&uncompressed_data),
                cli_parse::Version::Two => compressed_data = compress_v2(&uncompressed_data),
                _ => {
                    eprintln!("Warning: Auto-choice failed, defaulting to V1.");
                    compressed_data = compress_v1(&uncompressed_data);
                }
            }
        }
    }

    let compressed_len = compressed_data.len();

    // CHECK THE STATS FLAG: Print enhanced compression statistics if enabled.
    if is_stats_enabled {
        let compression_ratio = compressed_len as f64 / uncompressed_len as f64;
        let compression_percentage = (1.0 - compression_ratio) * 100.0;
        let bytes_saved = uncompressed_len as i64 - compressed_len as i64;

        println!("\n--- Compression Statistics ---");
        println!("  Version Used:      {}", version);
        println!("  Original Size:     {} bytes", uncompressed_len);
        println!("  Compressed Size:   {} bytes", compressed_len);
        println!("  Bytes Saved:       {} bytes", bytes_saved);
        println!(
            "  Compression Ratio: {:.3} (Compressed / Original)",
            compression_ratio
        );
        println!("  Compression %:     {:.2}%", compression_percentage);
    }

    // Write the file
    let mut compressed_data_file = match std::fs::File::create(output_file_path.clone()) {
        Ok(file) => file,
        Err(e) => {
            eprintln!(
                "Error creating output file {}: {}",
                output_file_path.to_string_lossy(),
                e
            );
            return;
        }
    };

    if let Err(e) = compressed_data_file.write_all(&compressed_data) {
        eprintln!("Error writing to output file: {}", e);
    } else {
        println!("Successfully wrote file: {:?}", output_file_path);
    }
}

fn decompress_from_file(
    input_file_path: PathBuf,
    output_file_path: PathBuf,
    version: cli_parse::Version,
    is_stats_enabled: bool,
) {
    if !input_file_path
        .to_string_lossy()
        .as_ref()
        .ends_with(".purgepack")
    {
        println!("Not a purgepack compressed file (missing .purgepack extension).");
        return;
    }

    let compressed_data = match std::fs::read(&input_file_path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error reading input file {:?}: {}", input_file_path, e);
            return;
        }
    };

    let compressed_len = compressed_data.len();

    let result_data = match version {
        Version::One => decompress_v1(&compressed_data),
        Version::Two => decompress_v2(&compressed_data),
        _ => decompress_v1(&compressed_data),
    };

    let decompressed_data = match result_data {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Decompression error: {}", e);
            return;
        }
    };

    let decompressed_len = decompressed_data.len();

    // Print enhanced decompression statistics if enabled.
    if is_stats_enabled {
        let expansion_ratio = decompressed_len as f64 / compressed_len as f64;
        let expansion_percentage = (expansion_ratio - 1.0) * 100.0;
        let bytes_restored = decompressed_len as i64 - compressed_len as i64;

        println!("\n--- Decompression Statistics ---");
        println!("  Version Used:      {}", version);
        println!("  Compressed Size:   {} bytes", compressed_len);
        println!("  Decompressed Size: {} bytes", decompressed_len);
        println!("  Bytes Restored:    {} bytes", bytes_restored);
        println!(
            "  Expansion Ratio:   {:.3} (Decompressed / Compressed)",
            expansion_ratio
        );
        println!("  Expansion %:       {:.2}%", expansion_percentage);
    }

    // Write the output file
    let mut decompressed_data_file = match std::fs::File::create(&output_file_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Error creating output file {:?}: {}", output_file_path, e);
            return;
        }
    };

    match decompressed_data_file.write_all(&decompressed_data) {
        Ok(_) => println!("Successfully written to {:?}", output_file_path),
        Err(e) => eprintln!("Error writing to file: {}", e),
    }
}

fn auto_choice_from_chunks(chunks: &Vec<Vec<u8>>) -> cli_parse::Version {
    let mut version1_score = 0;
    let mut version2_score = 0;

    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }

        let compressed_data_v1 = compress_v1(chunk);
        let compressed_data_v2 = compress_v2(chunk);

        if compressed_data_v2.len() < compressed_data_v1.len() {
            version2_score += 1;
        } else if compressed_data_v1.len() < compressed_data_v2.len() {
            version1_score += 1;
        }
    }

    if version2_score > version1_score {
        cli_parse::Version::Two
    } else {
        cli_parse::Version::One
    }
}

fn read_multiple_random_chunks(file_path: &PathBuf) -> io::Result<Vec<Vec<u8>>> {
    let mut file = File::open(file_path)?;
    let file_size = file.metadata()?.len();

    if file_size == 0 {
        return Ok(Vec::new());
    }

    let chunk_size = CHUNK_SIZE_BYTES as u64;
    let mut chunks: Vec<Vec<u8>> = Vec::with_capacity(NUM_CHUNKS);
    let mut rng = rand::rng();

    let max_start_offset = if file_size < chunk_size {
        0
    } else {
        file_size - chunk_size
    };

    if file_size <= chunk_size {
        let mut buffer = Vec::with_capacity(file_size as usize);
        file.read_to_end(&mut buffer)?;
        chunks.push(buffer);
        return Ok(chunks);
    }

    for _ in 0..NUM_CHUNKS {
        let random_offset = rng.random_range(0..=max_start_offset);

        file.seek(io::SeekFrom::Start(random_offset))?;

        let mut buffer = vec![0; CHUNK_SIZE_BYTES];
        file.read_exact(&mut buffer)?;

        chunks.push(buffer);
    }

    Ok(chunks)
}
