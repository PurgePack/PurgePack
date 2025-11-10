use clap::{Parser, Subcommand, ValueEnum};
use std::{ffi::OsString, io::Write, path::PathBuf};

use shared_files::core_header;

const MAX_RUN_LENGTH: u8 = u8::MAX;
const ESCAPE_BYTE: u8 = u8::MIN;

/// Defines which specialized Run-Length Encoding (RLE) algorithm version
/// the program should use for compression or decompression.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Version {
    /// RLE v1: Optimized for highly compressible data (many long runs).
    #[value(name = "1")]
    One,
    /// RLE v2: Optimized for less compressible data (fewer, shorter runs).
    #[value(name = "2")]
    Two,
    /// The program automatically selects the most appropriate algorithm.
    #[value(name = "auto")]
    Auto,
}

/// Implements the Display trait to allow the Version enum to be converted
/// into a user-readable string (e.g., "1", "2", or "auto").
/// This is required for clap to correctly display the default value in the help message.
impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Version::One => write!(f, "1"),
            Version::Two => write!(f, "2"),
            Version::Auto => write!(f, "auto"),
        }
    }
}

/// The main operations available for the utility.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Compresses the specified input file to the given output path.
    #[clap(alias = "c")] // Allows 'c' as a short alias for 'compress'
    Compress {
        /// The file path to read data from for compression. This must exist.
        input_file: PathBuf,
        /// The file path to write the compressed data to.
        output_file: PathBuf,
    },

    /// Decompresses the specified input file to the given output path.
    #[clap(alias = "d")] // Allows 'd' as a short alias for 'decompress'
    Decompress {
        /// The file path to read data from for decompression.
        input_file: PathBuf,
        /// The file path to write the decompressed data to.
        output_file: PathBuf,
    },
}

/// The main command line argument structure for the RLE Compression Utility.
/// This handles global options and delegates file arguments to the subcommands (compress/decompress).
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "RLE Compression Utility.",
    long_about = "A utility for compression and decompression using specialized Run-Length Encoding (RLE) versions."
)]
pub struct CliArgs {
    /// The primary operation (compress or decompress) and its file paths.
    #[command(subcommand)]
    pub command: Commands,
    /// Enables statistics output, such as compression ratio and execution time.
    #[arg(short, long)]
    pub stats: bool,
    /// Specifies the RLE algorithm version to run. Possible values: "1", "2", or "auto".
    #[arg(short = 'r', long = "rle-version", default_value_t = Version::Auto)]
    pub rle_version: Version,
}

impl CliArgs {
    /// Validates the command line arguments after parsing, specifically ensuring:
    /// 1. The input file exists and is a file.
    /// 2. The parent directory for the output file exists and is a directory.
    pub fn validate(&self) -> Result<(), CliError> {
        let (in_path, out_path) = match &self.command {
            Commands::Compress {
                input_file,
                output_file,
            } => (input_file, output_file),
            Commands::Decompress {
                input_file,
                output_file,
            } => (input_file, output_file),
        };

        // --- Input File Validation ---
        if !in_path.exists() {
            return Err(CliError::InputFileNotFound(in_path.clone()));
        }
        if !in_path.is_file() {
            return Err(CliError::InputNotFile(in_path.clone()));
        }

        // --- Output Directory Validation ---
        if let Some(parent) = out_path.parent() {
            if !parent.exists() {
                return Err(CliError::OutputParentDirNotFound(parent.to_path_buf()));
            }
            if !parent.is_dir() {
                return Err(CliError::OutputParentNotDir(parent.to_path_buf()));
            }
        }

        Ok(())
    }
}

// Possible errors encountered during command line argument processing,
/// file validation, or when executing the RLE operations.
#[derive(Debug)]
pub enum CliError {
    /// The specified run-length or count was outside the valid bounds.
    InvalidLength,
    /// An unexpected or unsupported operation was attempted during processing.
    InvalidOperation(String),
    /// The specified input file could not be found.
    InputFileNotFound(PathBuf),
    /// The specified input path exists, but is not a file.
    InputNotFile(PathBuf),
    /// The parent directory for the output file does not exist.
    OutputParentDirNotFound(PathBuf),
    /// The parent path for the output file exists, but is not a directory.
    OutputParentNotDir(PathBuf),
    /// An invalid RLE version string was provided.
    InvalidVersion(String),
    /// An error originating directly from the argument parsing library (clap).
    ClapError(clap::Error),
}

// Allows for seamless conversion of a `clap::Error` directly into a `CliError`.
/// This is typically used when handling the result of `CliArgs::parse()`.
impl From<clap::Error> for CliError {
    fn from(error: clap::Error) -> Self {
        CliError::ClapError(error)
    }
}

#[unsafe(no_mangle)]
extern "system" fn module_startup(_core: &core_header::CoreH) {
    match parse_args() {
        Ok(args) => {
            println!("Valid argument configuration loaded: {:?}", args);
            match args.command {
                Commands::Compress {
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

                    compress_from_file(
                        input_file.into_os_string(),
                        output_file.into_os_string(),
                        args.rle_version.to_string().into(),
                        args.stats,
                    );
                }
                Commands::Decompress {
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

                    decompress_from_file(
                        input_file.into_os_string(),
                        output_file.into_os_string(),
                        args.rle_version.to_string().into(),
                        args.stats,
                    );
                }
            }
        }
        Err(CliError::ClapError(e)) => {
            e.exit();
        }
        Err(e) => {
            println!("Error during argument validation:");
            match e {
                CliError::InputFileNotFound(path) => {
                    println!("Error: Input file does not exist: {}", path.display());
                }
                CliError::InputNotFile(path) => {
                    println!("Error: Input path is not a file: {}", path.display());
                }
                CliError::OutputParentDirNotFound(path) => {
                    println!(
                        "Error: The output directory does not exist: {}",
                        path.display()
                    );
                    println!("Please ensure the directory is created: {}", path.display());
                }
                CliError::OutputParentNotDir(path) => {
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
    input_file_path: OsString,
    output_file_path: OsString,
    version: OsString,
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
    let mut version_used = version.to_string_lossy().to_string();

    let uncompressed_len = uncompressed_data.len();

    // Determine which version to use and execute compression
    match version_used.as_ref() {
        "v1" => compressed_data = compress_v1(&uncompressed_data),
        "v2" => compressed_data = compress_v2(&uncompressed_data),
        "auto" => {
            let choice = auto_choice(&uncompressed_data);
            version_used = choice.to_string();
            match choice {
                "v1" => compressed_data = compress_v1(&uncompressed_data),
                "v2" => compressed_data = compress_v2(&uncompressed_data),
                _ => {
                    eprintln!("Warning: Auto-choice failed, defaulting to V1.");
                    version_used = "v1".to_string();
                    compressed_data = compress_v1(&uncompressed_data);
                }
            }
        }
        _ => {
            eprintln!("Warning: Unknown version '{}', using V1.", version_used);
            version_used = "v1".to_string();
            compressed_data = compress_v1(&uncompressed_data);
        }
    }

    let compressed_len = compressed_data.len();

    // CHECK THE STATS FLAG: Print enhanced compression statistics if enabled.
    if is_stats_enabled {
        let compression_ratio = compressed_len as f64 / uncompressed_len as f64;
        let compression_percentage = (1.0 - compression_ratio) * 100.0;
        let bytes_saved = uncompressed_len as i64 - compressed_len as i64;

        println!("\n--- Compression Statistics ---");
        println!("  Version Used:      {}", version_used);
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
        println!(
            "Successfully wrote file: {}",
            output_file_path.to_string_lossy()
        );
    }
}

fn decompress_from_file(
    input_file_path: OsString,
    output_file_path: OsString,
    version: OsString,
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
    let version_str = version.to_string_lossy().to_string();

    let result_data = match version_str.as_ref() {
        "v1" => decompress_v1(&compressed_data),
        "v2" => decompress_v2(&compressed_data),
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
        println!("  Version Used:      {}", version_str);
        println!("  Compressed Size:   {} bytes", compressed_len);
        println!("  Decompressed Size: {} bytes", decompressed_len);
        println!("  Bytes Restored:    {} bytes", bytes_restored);
        println!(
            "  Expansion Ratio:   {:.3} (Decompressed / Compressed)",
            expansion_ratio
        );
        println!("  Expansion %:       {:.2}%", expansion_percentage);
    }

    // Attempt to convert to UTF-8 if output is .txt
    if output_file_path
        .to_string_lossy()
        .as_ref()
        .ends_with(".txt")
    {
        let _ = String::from_utf8(decompressed_data.clone()).unwrap();
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

fn auto_choice(uncompressed_data: &Vec<u8>) -> &'static str {
    let compressed_data_v1 = compress_v1(uncompressed_data);
    let compressed_data_v2 = compress_v2(uncompressed_data);

    if compressed_data_v1.len() >= compressed_data_v2.len() {
        return "v2";
    } else {
        return "v1";
    }
}

pub fn parse_args() -> Result<CliArgs, CliError> {
    let args = CliArgs::try_parse()?;
    args.validate()?;
    Ok(args)
}
