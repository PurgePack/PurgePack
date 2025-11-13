use std::{
    fs::File,
    io::{self, BufRead, Read, Write},
    path::{self},
};
mod cli_parse;
use shared_files::core_header::{self};

/// The direction of the transformation (Encode or Decode).
#[derive(Debug, Clone, Copy)]
enum Transform {
    /// Applies delta encoding (current byte - previous byte). Used for Transformation.
    Encode,
    /// Applies delta decoding (current byte + previous byte). Used for inverse transformation.
    Decode,
}

/// Magic bytes to identify the PurgePack application. PPCB stands for "PurgePack Compressed Binary".
const APPLICATION_MAGIC: [u8; 4] = *b"PPCB";
/// Module ID (Algorithm Identifier) for the current First-Order Delta Encoding/Decoding.
const MODULE_ID: u8 = 0x01;
/// The size of the header in bytes (4 bytes for magic + 1 byte for module ID).
const HEADER_SIZE: u64 = 5;
// The PurgePack header contains a magic number (4 bytes) and a module ID (1 byte).
struct PurgePackHeader {
    application_magic: [u8; 4],
    module_id: u8,
}
// The file extension for PurgePack Compressed Binary (PPCB) files.
const FILE_EXTENSION: &str = "ppcb";

/// The main entry point for the module when it is started.
///
/// This function is responsible for:
/// 1. Parsing and validating command-line arguments via the `cli_parse` module.
/// 2. Determining the requested operation (Encode or Decode) based on the command.
/// 3. Initiating the file processing via `start_proccessing_file`.
/// 4. Handling and reporting any CLI parsing or file processing errors.
#[unsafe(no_mangle)]
extern "C" fn module_startup(_core: &core_header::CoreH, args: &mut Vec<String>) {
    args.insert(0, "dummy_program_name".to_string());
    match cli_parse::parse_args(&args) {
        Ok(args) => match args.command {
            cli_parse::Commands::Transform(args) => {
                println!(
                    "Transform: Input: {}, Output: {}",
                    args.input_file.display(),
                    args.output_file.display()
                );
                println!(
                    "Transform: Statistics: {}",
                    if args.stats { "Enabled" } else { "Disabled" }
                );
                let transform_type = Transform::Encode;
                match start_proccessing_file(
                    args.input_file,
                    args.output_file,
                    transform_type,
                    args.stats,
                ) {
                    Ok(()) => println!("Transform: Success"),
                    Err(e) => println!("Transform: Error: {}", e),
                }
            }
            cli_parse::Commands::Inverse(args) => {
                println!(
                    "Inverse: Input: {}, Output: {}",
                    args.input_file.display(),
                    args.output_file.display()
                );
                println!(
                    "Inverse: Statistics: {}",
                    if args.stats { "Enabled" } else { "Disabled" }
                );
                let transform_type = Transform::Decode;
                match start_proccessing_file(
                    args.input_file,
                    args.output_file,
                    transform_type,
                    args.stats,
                ) {
                    Ok(()) => println!("Inverse: Success"),
                    Err(e) => println!("Inverse: Error: {}", e),
                }
            }
        },
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

/// The shutdown function for the module.
#[unsafe(no_mangle)]
extern "C" fn module_shutdown(_core: &core_header::CoreH) {
    println!("Delta encoder module shutting down.");
}
/// Initializes the file handles and coordinates the chunk-by-chunk delta transformation.
///
/// This function opens the input and output files, handles the initial "seed" byte,
/// and then loops, reading the input file in buffered chunks (`fill_buf`) and
/// passing them to `transform_data_chunk`.
///
/// # Arguments
///
/// * `input_file` - The path to the source file.
/// * `output_file` - The path to the destination file.
/// * `transform_type` - The direction of the operation (`Encode` or `Decode`).
/// * `_stats` - A boolean flag for statistics calculation (currently unused).
///
/// # Errors
///
/// Returns an `io::Error` if file opening fails, reading/writing fails, or
/// flushing the buffer fails.
fn start_proccessing_file(
    input_file: path::PathBuf,
    mut output_file: path::PathBuf,
    transform_type: Transform,
    _stats: bool,
) -> Result<(), io::Error> {
    if let Transform::Decode = transform_type {
        let has_correct_extension = input_file.extension().map_or(false, |ext| {
            ext.to_string_lossy().eq_ignore_ascii_case(FILE_EXTENSION)
        });

        if !has_correct_extension {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Input file must have the '{}' extension for decoding. Found: {}",
                    FILE_EXTENSION,
                    input_file.display()
                ),
            ));
        }
    }
    if let Transform::Encode = transform_type {
        // If the output path has no extension, append the required .ppcb extension.
        // This ensures the encoded file is correctly labeled for later decoding.
        if output_file.extension().is_none() {
            output_file.set_extension(FILE_EXTENSION);
            println!(
                "Encode: Automatic extension '{}' placed on output file: {}",
                FILE_EXTENSION,
                output_file.display()
            );
        }
    }
    let input = File::open(input_file)?;
    let output = File::create(output_file)?;
    let mut buff_reader = std::io::BufReader::new(input);
    let mut buff_writer = std::io::BufWriter::new(output);
    let mut previous_byte: u8;

    match transform_type {
        Transform::Encode => write_header(&mut buff_writer)?,
        Transform::Decode => {
            // this variable might be usefull in the future if multiple versions present
            let _module_id = read_and_validate_header(&mut buff_reader)?;
        }
    }

    previous_byte = match set_delta_seed(&mut buff_reader, &mut buff_writer) {
        Ok(Some(value)) => value,
        Ok(None) => {
            buff_writer.flush()?;
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    loop {
        let current_chunk = buff_reader.fill_buf()?;
        let chunk_length = current_chunk.len();
        if current_chunk.is_empty() {
            break;
        }
        previous_byte = transform_data_chunk(
            current_chunk,
            &mut buff_writer,
            previous_byte,
            transform_type,
        )?;
        buff_reader.consume(chunk_length);
    }

    buff_writer.flush()?;
    Ok(())
}
/// Performs the delta encoding or decoding on a single chunk of data.
///
/// The transformation is done byte-by-byte, with the result of each step
/// depending on the calculated value of the previous byte. The operation uses
/// **wrapping arithmetic** (`wrapping_sub`/`wrapping_add`) to prevent panic on
/// overflow/underflow. We treat the bytes as cyclic unsigned 8-bit integers (`u8`),
/// where the valid range is $0$ to $255$. This means we avoid signed values;
/// for example, a subtraction that results in $-3$ (like $12-15$) automatically wraps to $253$,
/// and an addition that overflows $255$ automatically wraps back towards $0$.
///
/// # Arguments
///
/// * `data` - The slice of bytes to be transformed (either original data or deltas).
/// * `buff_writer` - The buffered writer to output the results.
/// * `previous_value` - The preceding value needed for the delta calculation (the seed).
/// * `transform_type` - The direction of the operation (`Encode` or `Decode`).
///
/// # Returns
///
/// The value of the last transformed byte, which serves as the seed for the
/// subsequent call or data chunk.
///
/// # Errors
///
/// Returns an `io::Error` if writing the transformed data fails.
/// /// ```rust
/// use std::io::{self, Cursor, BufWriter, Write};
///
/// // Internal types and helper to test the logic without file creation.
/// #[derive(Debug, Clone, Copy)]
/// enum Transform { Encode, Decode }
///
/// fn transform_chunk_logic<W: Write>(
///     data: &[u8],
///     buff_writer: &mut BufWriter<W>,
///     mut previous_value: u8,
///     transform_type: Transform,
/// ) -> io::Result<u8> {
///     for &current_byte in data.iter() {
///         let delta_change = match transform_type {
///             Transform::Encode => current_byte.wrapping_sub(previous_value),
///             Transform::Decode => current_byte.wrapping_add(previous_value),
///         };
///         buff_writer.write_all(&[delta_change])?;
///
///         match transform_type {
///             Transform::Encode => { previous_value = current_byte; }
///             Transform::Decode => previous_value = delta_change,
///         }
///     }
///     Ok(previous_value)
/// }
///
/// let original_data: Vec<u8> = vec![15, 12, 16];
/// let initial_seed: u8 = 10;
///
/// // 1. Encode: [15, 12, 16] -> [5, 253, 4] (Delta bytes)
/// let mut encoded_output = Cursor::new(Vec::new());
/// let mut encoded_writer = BufWriter::new(&mut encoded_output);
/// let final_seed_encode = transform_chunk_logic(
///     &original_data,
///     &mut encoded_writer,
///     initial_seed,
///     Transform::Encode,
/// )?;
/// encoded_writer.flush()?;
/// let delta_bytes = encoded_output.into_inner();
///
/// assert_eq!(delta_bytes, vec![5, 253, 4]);
/// assert_eq!(final_seed_encode, 16);
///
/// // 2. Decode: [5, 253, 4] -> [15, 12, 16] (Original bytes recovered)
/// let mut decoded_output = Cursor::new(Vec::new());
/// let mut decoded_writer = BufWriter::new(&mut decoded_output);
/// let final_seed_decode = transform_chunk_logic(
///     &delta_bytes,
///     &mut decoded_writer,
///     initial_seed,
///     Transform::Decode,
/// )?;
/// decoded_writer.flush()?;
/// let decoded_bytes = decoded_output.into_inner();
///
/// assert_eq!(decoded_bytes, original_data);
/// assert_eq!(final_seed_decode, 16);
/// # Ok::<(), io::Error>(())
/// ```
fn transform_data_chunk(
    data: &[u8],
    buff_writer: &mut std::io::BufWriter<File>,
    mut previous_value: u8,
    transform_type: Transform,
) -> io::Result<u8> {
    for &current_byte in data.iter() {
        let delta_change = match transform_type {
            Transform::Encode => current_byte.wrapping_sub(previous_value),
            Transform::Decode => current_byte.wrapping_add(previous_value),
        };
        buff_writer.write_all(&[delta_change])?;

        match transform_type {
            Transform::Encode => {
                previous_value = current_byte;
            }
            Transform::Decode => previous_value = delta_change,
        }
    }

    Ok(previous_value)
}

// Reads the first byte from the input stream and writes it directly to the output stream.
///
/// This first byte acts as the delta seed for the rest of the transformation process.
///
/// # Arguments
///
/// * `buff_reader` - The buffered reader for the input file.
/// * `buff_writer` - The buffered writer for the output file.
///
/// # Returns
///
/// Returns `Ok(Some(u8))` containing the seed byte, or `Ok(None)` if the input file
/// was empty.
///
/// # Errors
///
/// Returns an `io::Error` if reading or writing the seed byte fails, unless the
/// error is `io::ErrorKind::UnexpectedEof` (which is treated as a successful end of file).
fn set_delta_seed(
    buff_reader: &mut std::io::BufReader<File>,
    buff_writer: &mut std::io::BufWriter<File>,
) -> Result<Option<u8>, io::Error> {
    let mut seed = [0u8; 1];
    match buff_reader.read_exact(&mut seed) {
        Ok(_) => {
            buff_writer.write_all(&seed)?;
            Ok(Some(seed[0]))
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}
/// Writes the PurgePack header (Magic Number and Module ID) to the output stream.
///
/// # Arguments
///
/// * `buff_writer` - The buffered writer for the output file.
///
/// # Returns
///
/// Returns `Ok(())` if the header is successfully written, or an `io::Error` if
/// writing the header fails.
fn write_header(buff_writer: &mut std::io::BufWriter<File>) -> Result<(), io::Error> {
    let header = PurgePackHeader {
        application_magic: APPLICATION_MAGIC,
        module_id: MODULE_ID,
    };
    buff_writer.write_all(&header.application_magic)?;
    buff_writer.write_all(&[header.module_id])?;
    Ok(())
}

/// Reads and validates the PurgePack header from the input stream.
/// Also determines the correct module ID to use for decoding.
///
/// # Arguments
///
/// * `buff_reader` - The buffered reader for the input file.
///
/// # Returns
///
/// Returns `Ok(u8)` containing the module ID, or an `io::Error` if reading or validating the header fails.
fn read_and_validate_header(buff_reader: &mut std::io::BufReader<File>) -> Result<u8, io::Error> {
    let mut header_bytes = [0u8; HEADER_SIZE as usize];
    buff_reader.read_exact(&mut header_bytes).map_err(|e| {
        io::Error::new(
            e.kind(),
            "Failed to read PurgePack header. File may be too short or corrupted.",
        )
    })?;
    let magic_number = [
        header_bytes[0],
        header_bytes[1],
        header_bytes[2],
        header_bytes[3],
    ];
    let module_id = header_bytes[4];
    if magic_number != APPLICATION_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid PurgePack magic number. This may not be a valid PurgePack Compressed Binary (PPCB) file.",
        ));
    }

    if module_id != MODULE_ID {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Unsupported module ID: 0x{:02X}. Only 0x{:02X} (Delta V1) is supported.",
                module_id, MODULE_ID
            ),
        ));
    }

    Ok(module_id)
}
