use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub struct CommonArgs {
    /// The path to the input file.
    pub input_file: PathBuf,
    /// The path where the output file will be written.
    pub output_file: PathBuf,
    /// Enables statistics output
    #[arg(short, long)]
    pub stats: bool,
}
/// The main operations available for the utility.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Executes the forward or inverse Delta Transform on a file.
    #[clap(alias = "t")]
    Transform(CommonArgs),
    /// Executes the inverse Delta Transform on a file.
    #[clap(alias = "i")]
    Inverse(CommonArgs),
}

/// The main command line argument structure for the Delta Transform Utility.
/// This delegates all responsibility to the subcommand since there are no global options.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Delta Transform Utility.",
    long_about = "A utility for performing forward and inverse Delta Transforms (e.g., differential coding) on binary or sequential data.",
    after_help = "
    COMMON USAGE:
      To use, start with the COMMAND ('transform'), followed by the INPUT and OUTPUT files.
      The '--stats' flag is optional and follows the file paths.

    EXAMPLES:
    # 1. Basic Delta Transform
    delta_tool.exe transform raw_data.bin transformed.dt

    # 2. Transforming and showing statistics (Note: -s comes AFTER the file paths)
    delta_tool.exe transform transformed.dt restored_data.bin -s

    # 3. Using the short alias for transform
    delta_tool.exe t source.bin dest.dt -s

    # 4. Inverse Delta Transform
    delta_tool.exe i transformed.dt restored_data.bin
"
)]
pub struct CliArgs {
    /// The primary operation (transform) and its associated arguments (including stats).
    #[command(subcommand)]
    pub command: Commands,
}

impl CliArgs {
    /// Validates the command line arguments after parsing, specifically ensuring:
    /// 1. The input file exists and is a file.
    /// 2. The parent directory for the output file exists and is a directory.
    pub fn validate(&self) -> Result<(), CliError> {
        let common_args = match &self.command {
            Commands::Transform(args) => args,
            Commands::Inverse(args) => args,
        };

        let in_path = &common_args.input_file;
        let out_path = &common_args.output_file;

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

/// Possible errors encountered during command line argument processing,
/// file validation, or when executing the Delta Transform operations.
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

/// Allows for seamless conversion of a `clap::Error` directly into a `CliError`.
/// This is typically used when handling the result of `CliArgs::parse()`.
impl From<clap::Error> for CliError {
    fn from(error: clap::Error) -> Self {
        CliError::ClapError(error)
    }
}

/// Allows for parsing command line arguments and validating them.
pub fn parse_args() -> Result<CliArgs, CliError> {
    let args = CliArgs::try_parse()?;
    args.validate()?;
    Ok(args)
}
