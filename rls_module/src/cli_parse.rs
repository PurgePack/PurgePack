use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
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
    long_about = "A utility for compression and decompression using specialized Run-Length Encoding (RLE) versions.",
    after_help = "
    COMMON USAGE:
      To use, start with the COMMAND (compress/decompress), followed by the INPUT and OUTPUT files.
      Options like '-s' (stats) and '-r' (version) go before the command.

    EXAMPLES:
    # 1. Basic Compression (uses 'auto' version selection by default)
    purgepack.exe compress my_data.txt my_data.ppcb

    # 2. Decompressing and showing statistics
    purgepack.exe decompress -s archive.ppcb restored.txt

    # 3. Compressing using RLE v2 algorithm
    purgepack.exe -r 2 compress huge_log.log huge_log.ppcb

    # 4. Using the short alias for compression
    purgepack.exe c source.bin dest.ppcb
"
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
    /// The specified input file could not be found.
    InputFileNotFound(PathBuf),
    /// The specified input path exists, but is not a file.
    InputNotFile(PathBuf),
    /// The parent directory for the output file does not exist.
    OutputParentDirNotFound(PathBuf),
    /// The parent path for the output file exists, but is not a directory.
    OutputParentNotDir(PathBuf),
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
/// Public function to parse and validate CLI arguments.
/// This is the entry point for argument handling from the main module.
pub fn parse_args() -> Result<CliArgs, CliError> {
    let args = CliArgs::try_parse()?;
    args.validate()?;
    Ok(args)
}
