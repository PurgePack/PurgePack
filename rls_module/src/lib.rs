use crate::cli_parse::Version;
use rand::Rng;
use shared_files::core_header;
use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    path::PathBuf,
    time::Instant,
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

    let mut versiom_chosen = version;
    println!("{:?}", version);
    let start_time = Instant::now();
    // Determine which version to use and execute compression
    match version {
        cli_parse::Version::One => compressed_data = compress_v1(&uncompressed_data),
        cli_parse::Version::Two => compressed_data = compress_v2(&uncompressed_data),
        cli_parse::Version::Auto => {
            let random_chunks = read_multiple_random_chunks(&input_file_path).unwrap();
            let choice = auto_choice_from_chunks(&random_chunks);
            versiom_chosen = choice;
            match choice {
                cli_parse::Version::One => compressed_data = compress_v1(&uncompressed_data),
                cli_parse::Version::Two => compressed_data = compress_v2(&uncompressed_data),
                cli_parse::Version::Auto => {
                    unreachable!(
                        "auto_choice_from_chunks function should never return unspecified version"
                    );
                }
            }
        }
    }
    let duration = start_time.elapsed();

    let compressed_len = compressed_data.len();
    if is_stats_enabled {
        print_statistics(
            versiom_chosen,
            uncompressed_len,
            compressed_len,
            duration,
            true,
        );
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
    version: Version,
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

    let start_time = Instant::now();

    let result_data = match version {
        Version::One => decompress_v1(&compressed_data),
        Version::Two => decompress_v2(&compressed_data),
        _ => decompress_v1(&compressed_data),
    };

    let duration = start_time.elapsed();

    let decompressed_data = match result_data {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Decompression error: {}", e);
            return;
        }
    };

    let decompressed_len = decompressed_data.len();

    if is_stats_enabled {
        print_statistics(version, compressed_len, decompressed_len, duration, false);
    }

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

/// Automatically chooses the preferred compression version based on the
/// compressibility analysis of input data chunks.
///
/// This function compares the effectiveness of two distinct compression
/// algorithms (Version 1 and Version 2) across a series of data chunks. It
/// selects the version that results in the smallest compressed output size
/// for the majority of the chunks.
///
/// # Arguments
///
/// * `chunks`: A reference to a `Vec<Vec<u8>>`, which contains the data
///   segments (chunks) to be processed. Each inner `Vec<u8>` represents a
///   separate chunk of data.
///
/// # Returns
///
/// * `cli_parse::Version`: The recommended compression version:
///   - `cli_parse::Version::Two`, if the V2 compression proved more effective on the majority of the chunks.
///   - `cli_parse::Version::One`, if the V1 compression was more effective, or in the case of a tie.
///
/// # Logic and Steps
///
/// 1. Initializes two counters (`version1_score`, `version2_score`) to zero.
/// 2. Iterates over every chunk in the `chunks` vector.
/// 3. Each non-empty chunk is compressed separately using the externally defined
///    functions `compress_v1()` and `compress_v2()`.
/// 4. Compares the resulting compressed lengths:
///    - If V2's output is shorter, `version2_score` is incremented.
///    - If V1's output is shorter, `version1_score` is incremented.
/// 5. After processing all chunks, the function returns the version with the
///    highest score. Version One is chosen in the event of a tie.
///
/// # Note on Tie-Breaking
///
/// **Version One is explicitly chosen in the event of a tie.**
///
/// For practical purposes and to ensure a non-tied result in most cases, **it is recommended**
/// to use an **odd number of segments (chunks)** when calling this function.
/// This maximizes the chance of a clear majority decision.
///
/// # Example (Assuming necessary definitions)
///
/// ```rust
/// // Assumed: enum Version { One, Two, ... }
/// // ...
///
/// // Javasolt páratlan számú darab (pl. 3, 5, 7, stb.)
/// let test_data = vec![
///     vec![0xAA, 0xAA, 0xAA],
///     vec![0x12, 0x34, 0x56],
///     vec![0xFF, 0x00, 0xFF],
/// ];
///
/// let chosen_version = auto_choice_from_chunks(&test_data);
/// ```
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

/// Reads multiple random-access chunks from the specified file path.
///
/// This function opens the file, determines its size, and then reads a
/// predefined number of data segments (NUM_CHUNKS) of a fixed size
/// (CHUNK_SIZE_BYTES) from random, non-overlapping starting positions
/// within the file.
///
/// Special Case: If the file size is less than or equal to CHUNK_SIZE_BYTES,
/// the entire file content is read and returned as a single chunk, overriding
/// the random selection process. If the file is empty, an empty vector is returned.
///
/// # Arguments
///
/// * `file_path`: A reference to a `&PathBuf`, representing the path to the
///   file from which the chunks will be read.
///
/// # Returns
///
/// * `io::Result<Vec<Vec<u8>>>`: An I/O result that contains:
///   - Success: A `Vec<Vec<u8>>` where each inner vector is a chunk of the
///     file data. The number of chunks is usually NUM_CHUNKS, and each chunk's
///     size is CHUNK_SIZE_BYTES (unless the file is smaller than one chunk).
///   - Error: An `io::Error` if the file cannot be opened, its metadata
///     cannot be read, or if an I/O operation (seek or read) fails.
///
/// # Logic and Steps
///
/// 1. File Opening and Size Check: Opens the file and retrieves its size.
///    If the size is 0, returns an empty vector immediately.
/// 2. Small File Handling: If the file size is less than or equal to
///    CHUNK_SIZE_BYTES, the entire content is read into a single buffer,
///    which is returned as the result.
/// 3. Random Offset Calculation: Determines the maximum allowed starting
///    offset (max_start_offset) to ensure a full CHUNK_SIZE_BYTES can always
///    be read from that position onward.
/// 4. Chunk Iteration: Loops NUM_CHUNKS times:
///    a. Generates a random starting offset between $0$ and max_start_offset.
///    b. Uses `file.seek()` to move the file pointer to the random offset.
///    c. Reads exactly CHUNK_SIZE_BYTES bytes into a new buffer using
///       `file.read_exact()`.
///    d. Appends the read buffer to the result vector.
/// 5. Final Result: Returns the vector containing all randomly read chunks.
///
/// # Assumed Constants
///
/// This function relies on two external constants defined in the scope:
///
/// * `CHUNK_SIZE_BYTES`: Defines the size of each chunk to be read (in bytes).
/// * `NUM_CHUNKS`: Defines the total number of chunks to read from the file.
///
/// Additionally, it requires a functional `rand::rng()` implementation for
/// generating the random starting offsets.
///
/// # Example (Using assumed constants)
///
/// ```rust
/// use std::path::PathBuf;
/// use std::io;
///
/// // Feltételezett konstansok
/// // const CHUNK_SIZE_BYTES: usize = 4096;
/// // const NUM_CHUNKS: usize = 5;
///
/// # fn read_multiple_random_chunks(file_path: &PathBuf) -> io::Result<Vec<Vec<u8>>> {
/// #    // ... (Függvény implementáció) ...
/// #    Ok(vec![vec![0; 4096]; 5])
/// # }
///
/// let path = PathBuf::from("data.bin");
/// match read_multiple_random_chunks(&path) {
///     Ok(chunks) => {
///         println!("Beolvasva {} darab adat.", chunks.len());
///         // A darabok feldolgozása...
///     }
///     Err(e) => {
///         eprintln!("Hiba a beolvasáskor: {}", e);
///     }
/// }
/// ```
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
/// Formats a byte count (`usize`) into a human-readable string using the
/// binary unit prefixes (powers of 1024, sometimes referred to as KiB/MiB,
/// but labeled here as KB/MB/etc.).
///
/// The output includes two decimal places for precision and the appropriate unit.
///
/// # Arguments
///
/// * `bytes` - The size in bytes (`usize`) to be formatted.
///
/// # Returns
///
/// A `String` containing the human-readable formatted size (e.g., "363.33 KB", "8.58 MB").
///
/// # Example
///
/// ```
/// let size_b = 512;
/// let size_mb = 5242880; // 5 MB
///
/// assert_eq!(format_bytes(size_b), "512.00 B");
/// assert_eq!(format_bytes(size_mb), "5.00 MB");
/// ```
fn format_bytes(bytes: usize) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut num = bytes as f64;
    let mut unit_index = 0;

    while num >= 1024.0 && unit_index < UNITS.len() - 1 {
        num /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", num, UNITS[unit_index])
}

/// Prints detailed statistics for a compression or decompression process.
///
/// Calculates the byte difference, compression ratio, percentage savings,
/// and the speed (MiB/s) based on the provided duration.
///
/// # Arguments
///
/// * `version_used` - The version of the compression or decompression algorithm used.
/// * `original_len` - The length of the initial input data in bytes (`usize`).
/// * `processed_len` - The length of the **compressed** data (if compressing)
///                      or the **decompressed** data (if decompressing) in bytes (`usize`).
/// * `duration` - The time taken for the processing (`std::time::Duration`).
/// * `is_compression` - A boolean indicating whether the statistics are for
///                      a compression (`true`) or decompression (`false`) operation.
///
/// # Example (Compression)
///
/// Compression of a highly redundant file, demonstrating a high compression ratio:
///
/// ```rust
/// use std::time::Duration;
///
/// // Assume `cli_parse::Version` implements `Display` and a helper like `format_bytes` is available.
/// let version = 1;
/// let original = 372054;
/// let compressed = 3648;
/// let duration = Duration::from_secs_f64(0.0002746);
/// let is_compression = true;
///
/// // print_statistics(version, original, compressed, duration, is_compression);
///
/// // expected output:
/// // --- Compression Statistics 📊 ---
/// //       Version Used:          1
/// //       Original Size:         363.33 KB
/// //       Compressed Size:       3.56 KB
/// //       Bytes Difference:      368406 (359.77 KB)
/// //       Compression Ratio:     101.988:1 (Original / Compressed)
/// //       Space Saved:           359.77 KB
/// //       Compression Savings :  99.02(%)
/// //       Processing Time:       0.000 seconds
/// //       Compression Speed      1292.13 MiB/s
/// ```
///
/// # Panics
///
/// This function does not panic unless the underlying `println!` macro encounters an IO error.
fn print_statistics(
    version_used: cli_parse::Version,
    original_len: usize,
    processed_len: usize,
    duration: std::time::Duration,
    is_compression: bool,
) {
    let (uncompressed_len, compressed_len) = if is_compression {
        (original_len, processed_len)
    } else {
        (processed_len, original_len)
    };

    let ratio_label = "Original";
    let compression_ratio_factor = uncompressed_len as f64 / compressed_len as f64;

    let raw_byte_difference = uncompressed_len as i64 - compressed_len as i64;
    let difference_bytes = raw_byte_difference.abs() as usize;

    let percentage_base = uncompressed_len as f64;
    let percentage_change = (difference_bytes as f64 / percentage_base) * 100.0;

    let speed_mib_s = (uncompressed_len as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();

    let speed_name = if is_compression {
        "Compression Speed"
    } else {
        "Decompression Speed"
    };
    let title_name = if is_compression {
        "Compression"
    } else {
        "Decompression"
    };

    let (savings_label, bytes_label) = if compressed_len < uncompressed_len {
        (
            format!("Compression Savings : {:.2}(%)", percentage_change),
            "Space Saved:".to_string(),
        )
    } else if compressed_len > uncompressed_len {
        (
            format!("File Bloat :          {:.2}(%)", percentage_change),
            "Space Wasted:".to_string(),
        )
    } else {
        (
            "File Size Change :    0.00% (No Change)".to_string(),
            "Bytes Difference:".to_string(),
        )
    };

    println!("\n--- {} Statistics 📊 ---", title_name);
    println!("    Version Used:         {}", version_used);
    println!(
        "    Original Size:        {}",
        format_bytes(uncompressed_len)
    );
    println!("    Compressed Size:      {}", format_bytes(compressed_len));

    println!(
        "    Bytes Difference:     {} ({})",
        raw_byte_difference,
        format_bytes(raw_byte_difference.abs() as usize)
    );

    println!(
        "    Compression Ratio:    {:.3}:1 ({ratio_label} / Compressed)",
        compression_ratio_factor
    );
    println!("    {:<21} {}", bytes_label, format_bytes(difference_bytes));
    println!("    {}", savings_label);

    println!(
        "    Processing Time:      {:.3} seconds",
        duration.as_secs_f64()
    );
    println!("    {:<21} {:.2} MiB/s", speed_name, speed_mib_s);
}
