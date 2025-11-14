//! # Performance and Stats Utility Module
//!
//! This module provides a comprehensive suite of utilities for **performance measurement**,
//! **statistical aggregation**, and **human-readable data formatting**, primarily designed
//! for applications involving data processing tasks like compression or decompression.
//!
//! ## Key Features
//!
//! * **Precision Timing**: The [`StatsTimer`] and [`SubSectionTimer`] structs offer
//!     accurate measurement of both total operation time and detailed, step-by-step
//!     processing durations.
//! * **Zero-Cost Optional Stats**: The [`OptinalStatsTimer`] allows performance tracking
//!     to be conditionally enabled or disabled at runtime without incurring any overhead
//!     when disabled.
//! * **Data Aggregation**: The [`CompressionStats`] struct collects and calculates all
//!     relevant metrics (e.g., **Compression Ratio**, **Processing Speed (MiB/s)**,
//!     and **Percentage Change**) for a complete operation.
//! * **Builder Pattern**: The [`CompressionStatsBuilder`] ensures that all necessary
//!     fields for statistics calculation are provided, returning a robust [`BuilderError`]
//!     if mandatory fields are missing.
//! * **Formatting**: Includes the `format_bytes` helper function and custom `Display`
//!     implementations for clear, human-readable terminal output of all collected data.
//!
//! ## Example Usage: Required and Optional Timing
//!
//! The following example demonstrates how to use the mandatory [`StatsTimer`] for overall
//! measurement and the flexible [`OptinalStatsTimer`] for detailed, conditional step timing.
//!
//! ```rust
//! use crate::stats::{
//!     StatsTimer, OptinalStatsTimer, CompressionStatsBuilder, SectionStats, BuilderError,
//!     CompressionStats
//! };
//! use std::time::Duration;
//!
//! /// Runs a data processing operation, collecting stats based on the 'is_stats_enabled' flag.
//! fn run_operation(input_data: &[u8], is_stats_enabled: bool) -> Result<CompressionStats, BuilderError> {
//!     // 1. Mandatory Overall Timer: Used to measure the total execution time.
//!     let mut overall_timer = StatsTimer::new();
//!     let original_len = input_data.len();
//!     
//!     // 2. Optional Section Timer: Used to track detailed steps only if stats are enabled.
//!     // This is zero-cost if 'is_stats_enabled' is false.
//!     let mut optional_timer = OptinalStatsTimer::new(is_stats_enabled);
//!
//!     // --- Step 1: Data Preparation (Optional Timing) ---
//!     // optional_timer.start_section returns Option<SubSectionTimer>
//!     let prep_timer = optional_timer.start_section("Data Prep");
//!     // ... perform data preparation ...
//!     optional_timer.add_section(prep_timer); // Handles the Option safely
//!
//!     // --- Step 2: Core Processing (Required Timing) ---
//!     let compression_timer = overall_timer.start_section("Core Processing");
//!     // Perform the main compression work here...
//!     let processed_data_len = original_len / 2; // Mock result
//!     overall_timer.add_section(compression_timer.end()); // Records the duration
//!
//!     // --- Step 3: Finalization (Optional Timing) ---
//!     let final_timer = optional_timer.start_section("Finalization");
//!     // ... perform finalization steps ...
//!     optional_timer.add_section(final_timer);
//!
//!     // 3. End Timers and Collect Results
//!     let (total_duration, required_sections) = overall_timer.end();
//!     let (_, optional_sections) = optional_timer.end();
//!     
//!     // Combine all collected section statistics
//!     let sections: Vec<SectionStats> = required_sections
//!         .into_iter()
//!         .chain(optional_sections.into_iter())
//!         .collect();
//!
//!     // 4. Build Final Statistics
//!     CompressionStatsBuilder::new()
//!         .algorithm_name("Huffman")
//!         .algorithm_id(1)
//!         .version_used(1)
//!         .original_len(original_len)
//!         .processed_len(processed_data_len)
//!         .duration(total_duration)
//!         .is_compression(true)
//!         .sections(sections)
//!         .build()
//! }
//!
//! fn main() {
//!     let data = vec![0; 1024 * 1024]; // 1 MiB
//!     
//!     // Run 1: Statistics enabled (gets full detailed section timing)
//!     let stats_full = run_operation(&data, true).unwrap();
//!     println!("{}", stats_full);
//!
//!     // Run 2: Statistics disabled (gets overall time and only required sections)
//!     let stats_minimal = run_operation(&data, false).unwrap();
//!     // println!("{}", stats_minimal);
//! }
//! ```
use std::error::Error;
use std::fmt::{self, Display};
use std::time::{Duration, Instant};
const KIB: usize = 1024;
const MIB: usize = KIB * 1024;
const GIB: usize = MIB * 1024;
const TIB: usize = GIB * 1024;
/// Formats a raw byte count into a human-readable string using binary prefixes (KiB, MiB, GiB, TiB).
///
/// This is an internal helper function that converts the input byte count (`usize`)
/// into the largest appropriate unit: **Tebibytes (TiB)**, Gigibytes (GiB), Mebibytes (MiB),
/// Kibibytes (KiB), or Bytes. It uses base 1024.
///
/// The output is formatted to two decimal places for KiB, MiB, GiB, and TiB units,
/// while the Bytes unit is displayed as a whole number.
///
/// # Arguments
///
/// * `bytes`: The number of bytes to format, provided as a `usize`.
///
/// # Returns
///
/// A [`String`] representing the human-readable size (e.g., "3.00 GiB", "5.00 TiB" or "512 Bytes").
///
/// # Examples
///
/// ```
/// # // Mock implementation of format_bytes for doc test environment
/// # const KIB: usize = 1024;
/// # const MIB: usize = KIB * 1024;
/// # const GIB: usize = MIB * 1024;
/// # const TIB: usize = GIB * 1024;
/// # fn format_bytes(bytes: usize) -> String {
/// #     if bytes >= TIB {
/// #         format!("{:.2} TiB", bytes as f64 / TIB as f64)
/// #     } else if bytes >= GIB {
/// #         format!("{:.2} GiB", bytes as f64 / GIB as f64)
/// #     } else if bytes >= MIB {
/// #         format!("{:.2} MiB", bytes as f64 / MIB as f64)
/// #     } else if bytes >= KIB {
/// #         format!("{:.2} KiB", bytes as f64 / KIB as f64)
/// #     } else {
/// #         format!("{} Bytes", bytes)
/// #     }
/// # }
/// // Less than 1 KiB
/// assert_eq!(format_bytes(512), "512 Bytes");
///
/// // Exactly 1 KiB
/// assert_eq!(format_bytes(1024), "1.00 KiB");
///
/// // A fractional MiB
/// assert_eq!(format_bytes(1572864), "1.50 MiB");
///
/// // Over 1 GiB
/// assert_eq!(format_bytes(3 * GIB), "3.00 GiB");
///
/// // Over 1 TiB (5 * 1024^4)
/// assert_eq!(format_bytes(5 * TIB), "5.00 TiB");
/// ```
fn format_bytes(bytes: usize) -> String {
    if bytes >= TIB {
        format!("{:.2} TiB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} Bytes", bytes)
    }
}

/// A custom error type used to represent failures during the construction
/// of an object using the builder pattern.
#[derive(Debug)]
pub enum BuilderError {
    /// Indicates that a required configuration field was never set on the builder.
    ///
    /// The string slice identifies the name of the missing field (e.g., "algorithm_name").
    MissingField(&'static str),
}

impl Display for BuilderError {
    /// Implements `Display` to allow the error to be formatted and printed cleanly.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuilderError::MissingField(field) => {
                write!(f, "Builder Error: Missing required field '{}'", field)
            }
        }
    }
}

impl Error for BuilderError {
    /// Implements `Error` to make this type fully compatible with Rust's standard error traits.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Since MissingField does not wrap another error, we return None.
        None
    }
}

/// Stores detailed statistics for a compression or decompression operation.
///
/// This structure encapsulates metadata (algorithm used, version) and
/// performance metrics (lengths, time, ratio, speed) related to a single
/// processing task.
#[derive(Debug, Clone)] // Added Clone for idiomatic use, assuming it's intended
pub struct CompressionStats {
    // --- Input and Metadata Fields ---
    /// The human-readable name of the algorithm used (e.g., "Run Length Encoding" or "Huffman Encoding").
    pub algorithm_name: &'static str,
    /// A unique numerical identifier for the algorithm.
    pub algorithm_id: u8,
    /// The specific version of the algorithm used for this run.
    pub version_used: u8,
    /// The length of the data **before** processing (in bytes).
    /// (Uncompressed size for compression, compressed size for decompression).
    pub original_len: usize,
    /// The length of the data **after** processing (in bytes).
    /// (Compressed size for compression, uncompressed size for decompression).
    pub processed_len: usize,
    /// The total time taken for the entire process.
    pub duration: Duration,
    /// True if the process was compression, false if it was decompression.
    pub is_compression: bool,

    /// A list of timed steps within the overall process, providing a detailed
    /// breakdown of time consumption.
    pub sections: Vec<SectionStats>,

    // --- Calculated Fields ---
    /// The compression ratio factor, calculated as `uncompressed_len / compressed_len`.
    ///
    /// A value of 2.0 means the output size is half of the original (2x compression).
    pub compression_ratio_factor: f64,
    /// The processing speed, calculated in Mebibytes per second (MiB/s).
    pub speed_mib_s: f64,
    /// The raw difference in bytes: `uncompressed_len - compressed_len`.
    ///
    /// The sign indicates the direction: Positive for savings, negative for size increase (bloat).
    pub raw_byte_difference: i64,
    /// The absolute percentage change in size relative to the uncompressed size.
    ///
    /// This value is always positive. Use [`CompressionStats::raw_byte_difference`] to find the direction.
    pub percentage_change: f64,
}

/// A struct to hold the name and duration for a specific processing step.
///
/// Used primarily within the [`CompressionStats::sections`] field.
#[derive(Debug, Clone)] // Added Clone for consistency
pub struct SectionStats {
    /// The descriptive name of the step (e.g., "Hashing" or "Header Write").
    pub name: String,
    /// The time taken for this specific step.
    pub duration: Duration,
}

impl SectionStats {
    /// Creates a new [`SectionStats`] instance.
    ///
    /// # Arguments
    ///
    /// * `name`: The descriptive name of the section.
    /// * `duration`: The time taken for the section.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::time::Duration;
    /// # #[derive(Debug)] pub struct SectionStats { name: String, duration: Duration }
    /// # impl SectionStats {
    /// #     pub fn new(name: &str, duration: Duration) -> Self { SectionStats { name: name.to_string(), duration } }
    /// # }
    /// let duration = Duration::from_micros(1500);
    /// let stats = SectionStats::new("Encoding Block", duration);
    /// ```
    pub fn new(name: &str, duration: Duration) -> Self {
        SectionStats {
            name: name.to_string(),
            duration,
        }
    }
}

impl Display for SectionStats {
    /// Implements `Display` to format [`SectionStats`] for clean terminal output.
    ///
    /// The output format is: `[Section Name] [Duration] seconds`.
    ///
    /// Example Output:
    /// `Initialization              0.002 seconds`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            // Format: Left-align name within 30 characters, then duration to 3 decimal places
            "{:<30} {:.3} seconds",
            self.name,
            self.duration.as_secs_f64()
        )
    }
}

/// A simple timer used to measure the duration of a specific code section.
///
/// It holds the section's name and returns the complete [`SectionStats`] upon stopping.
///
/// This timer consumes itself when stopped, preventing double-timing.
///
/// # Example
///
/// ```ignore
/// let timer = SubSectionTimer::new("data_loading");
/// // ... code block to be timed ...
/// let stats = timer.end();
/// // The `timer` is consumed and can no longer be used.
/// ```
pub struct SubSectionTimer {
    start_time: Instant,
    section_name: String,
}

impl SubSectionTimer {
    /// Creates a new timer, immediately recording the current time as the start of measurement.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the section being measured.
    pub fn new(name: &str) -> Self {
        SubSectionTimer {
            start_time: Instant::now(),
            section_name: name.to_string(),
        }
    }

    /// Stops the timer and returns the complete [`SectionStats`] (name and duration).
    ///
    /// This method **consumes** `self`, guaranteeing the timer can only be ended once.
    ///
    /// # Returns
    ///
    /// A [`SectionStats`] struct containing the section name and elapsed time.
    pub fn end(self) -> SectionStats {
        let duration = self.start_time.elapsed();
        SectionStats::new(&self.section_name, duration)
    }
}

/// The main performance timer, which measures the overall program time and collects statistics from sub-sections.
///
/// This struct allows you to track the overall process duration and aggregate the results
/// of any completed [`SubSectionTimer`] instances.
pub struct StatsTimer {
    /// The start time of the entire process.
    start_time: Instant,
    /// A vector of all completed subsection statistics.
    sections: Vec<SectionStats>,
}

impl StatsTimer {
    /// Initializes and starts the main timer. This is the initial timestamp for the entire process.
    pub fn new() -> Self {
        StatsTimer {
            start_time: Instant::now(),
            sections: Vec::new(),
        }
    }

    /// Starts a new section timer, which should be stored in a local variable.
    ///
    /// The returned [`SubSectionTimer`] can be used to measure the duration of a specific
    /// block of code.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the section to be measured.
    ///
    /// # Returns
    ///
    /// A new [`SubSectionTimer`] instance.
    pub fn start_section(&mut self, name: &str) -> SubSectionTimer {
        SubSectionTimer::new(name)
    }

    /// Adds a completed [`SectionStats`] result to the internal collection.
    ///
    /// This is typically called by passing in the result of a `SubSectionTimer::end()` call.
    ///
    /// # Arguments
    ///
    /// * `section_stats`: The statistics for the completed section.
    pub fn add_section(&mut self, section_stats: SectionStats) {
        self.sections.push(section_stats);
    }

    /// Stops the overall timing and returns the total duration and all collected section statistics.
    ///
    /// This method **consumes** `self`.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// 1. The **total runtime** (`Duration`).
    /// 2. The **collected section statistics** (`Vec<SectionStats>`).
    pub fn end(self) -> (Duration, Vec<SectionStats>) {
        (self.start_time.elapsed(), self.sections)
    }
}

/// A wrapper struct that holds either a real StatsTimer or nothing (None).
///
/// It provides the same methods as StatsTimer but is entirely zero-cost and
/// performs no operations when statistics are disabled (i.e., when the internal
/// timer is None).
pub struct OptinalStatsTimer(Option<StatsTimer>);

impl OptinalStatsTimer {
    /// Creates a new timer that is either enabled or disabled.
    ///
    /// # Arguments
    ///
    /// * `enabled`: If true, an active StatsTimer is created. If false, the
    /// internal timer is None, and all method calls become no-ops.
    pub fn new(enabled: bool) -> Self {
        if enabled {
            OptinalStatsTimer(Some(StatsTimer::new()))
        } else {
            OptinalStatsTimer(None)
        }
    }

    /// Starts a section timer.
    ///
    /// Returns a SubSectionTimer wrapped in an Option if the timer is enabled,
    /// otherwise returns None immediately.
    pub fn start_section(&mut self, name: &str) -> Option<SubSectionTimer> {
        // This is where the .as_mut().map() logic is cleanly handled.
        self.0.as_mut().map(|t| t.start_section(name))
    }

    /// Adds a completed section result to the main timer.
    ///
    /// This method handles the Option<SubSectionTimer> safely and only records
    /// the result if the main timer was active and a SubSectionTimer was provided.
    ///
    /// If `timer` is `None`
    pub fn add_section(&mut self, timer: Option<SubSectionTimer>) {
        if let Some(sub_timer) = timer {
            if let Some(main_t) = self.0.as_mut() {
                main_t.add_section(sub_timer.end());
            }
        }
    }

    /// Stops the overall timing and returns the final results.
    ///
    /// This method consumes the wrapper struct. If the timer was disabled,
    /// it returns a default tuple: (zero duration, empty sections).
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// 1. The total runtime (`Duration`).
    /// 2. The collected section statistics (`Vec<SectionStats>`).
    pub fn end(self) -> (Duration, Vec<SectionStats>) {
        // If Some(t), returns t.end(). If None, returns the default tuple.
        self.0
            .map_or((Duration::from_secs(0), Vec::new()), |t| t.end())
    }
}

/// Builder for constructing [`CompressionStats`] using the method chaining pattern.
///
/// The builder ensures all required fields are provided before computing the final
/// statistics with the [`CompressionStatsBuilder::build`] method.
#[derive(Default)]
pub struct CompressionStatsBuilder {
    algorithm_name: Option<&'static str>,
    algorithm_id: Option<u8>,
    version_used: Option<u8>,
    original_len: Option<usize>,
    processed_len: Option<usize>,
    duration: Option<Duration>,
    is_compression: Option<bool>,
    sections: Vec<SectionStats>,
}

impl CompressionStats {
    /// Internal function to calculate all derived statistics from collected raw inputs.
    ///
    /// This method is called by [`CompressionStatsBuilder::build`] after all
    /// mandatory fields have been verified.
    fn calculate_stats(
        algorithm_name: &'static str,
        algorithm_id: u8,
        version_used: u8,
        original_len: usize,
        processed_len: usize,
        duration: Duration,
        is_compression: bool,
        sections: Vec<SectionStats>,
    ) -> Self {
        // --- LOGIC REMAINS UNCHANGED ---
        let (uncompressed_len, compressed_len) = if is_compression {
            (original_len, processed_len)
        } else {
            (processed_len, original_len)
        };

        let compression_ratio_factor = if compressed_len == 0 {
            0.0
        } else {
            uncompressed_len as f64 / compressed_len as f64
        };

        let duration_secs = duration.as_secs_f64();
        let speed_mib_s = if duration_secs == 0.0 {
            f64::INFINITY
        } else {
            (uncompressed_len as f64 / (1024.0 * 1024.0)) / duration_secs
        };

        let raw_byte_difference = uncompressed_len as i64 - compressed_len as i64;
        let difference_bytes = raw_byte_difference.abs() as usize;
        let percentage_base = uncompressed_len as f64;
        let percentage_change = if percentage_base == 0.0 {
            0.0
        } else {
            (difference_bytes as f64 / percentage_base) * 100.0
        };

        CompressionStats {
            algorithm_name,
            algorithm_id,
            version_used,
            original_len,
            processed_len,
            duration,
            is_compression,
            sections,
            compression_ratio_factor,
            speed_mib_s,
            raw_byte_difference,
            percentage_change,
        }
    }
}

impl CompressionStatsBuilder {
    /// Creates a new, empty builder.
    ///
    /// All fields are initialized to [`None`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::time::Duration;
    /// # use std::fmt;
    /// # #[derive(Debug)] pub enum BuilderError { MissingField(&'static str) }
    /// # impl fmt::Display for BuilderError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "") } }
    /// # impl std::error::Error for BuilderError {}
    /// # #[derive(Default)] pub struct CompressionStatsBuilder { algorithm_name: Option<&'static str>, algorithm_id: Option<u8>, version_used: Option<u8>, original_len: Option<usize>, processed_len: Option<usize>, duration: Option<Duration>, is_compression: Option<bool>, sections: Vec<u8> }
    /// # impl CompressionStatsBuilder { pub fn new() -> Self { Self::default() } }
    /// let builder = CompressionStatsBuilder::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the algorithm name.
    pub fn algorithm_name(mut self, name: &'static str) -> Self {
        self.algorithm_name = Some(name);
        self
    }
    /// Sets the algorithm ID.
    pub fn algorithm_id(mut self, id: u8) -> Self {
        self.algorithm_id = Some(id);
        self
    }
    /// Sets the version number of the algorithm used.
    pub fn version_used(mut self, version: u8) -> Self {
        self.version_used = Some(version);
        self
    }
    /// Sets the original, pre-processed byte length.
    pub fn original_len(mut self, len: usize) -> Self {
        self.original_len = Some(len);
        self
    }
    /// Sets the processed, output byte length.
    pub fn processed_len(mut self, len: usize) -> Self {
        self.processed_len = Some(len);
        self
    }
    /// Sets the total operation duration.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
    /// Sets whether the statistics are for compression (`true`) or decompression (`false`).
    pub fn is_compression(mut self, is_comp: bool) -> Self {
        self.is_compression = Some(is_comp);
        self
    }
    /// Sets the optional list of [`SectionStats`] for detailed timing.
    pub fn sections(mut self, sections: Vec<SectionStats>) -> Self {
        self.sections = sections;
        self
    }
    /// Adds a single [`SectionStats`] entry to the internal list of sections.
    ///
    /// This method returns `Self` to allow for convenient method chaining.
    pub fn add_section(mut self, name: &str, duration: Duration) -> Self {
        self.sections.push(SectionStats::new(name, duration));
        self
    }

    /// Attempts to build the final [`CompressionStats`] struct.
    ///
    /// If all mandatory fields are set, it calculates all derived statistics and
    /// returns the ready-to-use [`CompressionStats`].
    ///
    /// # Errors
    ///
    /// Returns an `Err(BuilderError)` if any required field is missing.
    pub fn build(self) -> Result<CompressionStats, BuilderError> {
        let name = self
            .algorithm_name
            .ok_or_else(|| BuilderError::MissingField("algorithm_name"))?;
        let id = self
            .algorithm_id
            .ok_or_else(|| BuilderError::MissingField("algorithm_id"))?;
        let version = self
            .version_used
            .ok_or_else(|| BuilderError::MissingField("version_used"))?;
        let original = self
            .original_len
            .ok_or_else(|| BuilderError::MissingField("original_len"))?;
        let processed = self
            .processed_len
            .ok_or_else(|| BuilderError::MissingField("processed_len"))?;
        let duration = self
            .duration
            .ok_or_else(|| BuilderError::MissingField("duration"))?;
        let is_comp = self
            .is_compression
            .ok_or_else(|| BuilderError::MissingField("is_compression"))?;

        Ok(CompressionStats::calculate_stats(
            name,
            id,
            version,
            original,
            processed,
            duration,
            is_comp,
            self.sections,
        ))
    }
}

// --- Display Trait for CompressionStats ---
impl Display for CompressionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (uncompressed_len, compressed_len) = if self.is_compression {
            (self.original_len, self.processed_len)
        } else {
            (self.processed_len, self.original_len)
        };
        let title_name = if self.is_compression {
            "Compression"
        } else {
            "Decompression"
        };
        let speed_name = if self.is_compression {
            "Compression Speed"
        } else {
            "Decompression Speed"
        };
        let raw_byte_difference_abs = self.raw_byte_difference.abs() as usize;
        let (savings_label, bytes_label) = if compressed_len < uncompressed_len {
            (
                format!("Compression Savings :  {:.2}(%)", self.percentage_change),
                "Space Saved:".to_string(),
            )
        } else if compressed_len > uncompressed_len {
            (
                format!("File Bloat :          {:.2}(%)", self.percentage_change),
                "Space Wasted:".to_string(),
            )
        } else {
            (
                "File Size Change :    0.00% (No Change)".to_string(),
                "Bytes Difference:".to_string(),
            )
        };

        // --- Summary Statistics ---
        writeln!(f, "\n--- {} Statistics 📊 ---", title_name)?;
        writeln!(f, "    Algorithm name:       {}", self.algorithm_name)?;
        writeln!(f, "    Algorithm ID:           {}", self.algorithm_id)?;
        writeln!(f, "    Version Used:         {}", self.version_used)?;
        writeln!(
            f,
            "    Original Size:        {}",
            format_bytes(uncompressed_len)
        )?;
        writeln!(
            f,
            "    Processed Size:      {}",
            format_bytes(compressed_len)
        )?;
        writeln!(
            f,
            "    Bytes Difference:     {} ({})",
            self.raw_byte_difference,
            format_bytes(raw_byte_difference_abs)
        )?;
        writeln!(
            f,
            "    Compression Ratio:    {:.3}:1 (Original / Processed)",
            self.compression_ratio_factor
        )?;
        writeln!(
            f,
            "    {:<21} {}",
            bytes_label,
            format_bytes(raw_byte_difference_abs)
        )?;
        writeln!(f, "    {}", savings_label)?;
        writeln!(
            f,
            "    Processing Time:      {:.3} seconds",
            self.duration.as_secs_f64()
        )?;
        write!(f, "    {:<21} {:.2} MiB/s", speed_name, self.speed_mib_s)?;

        // --- Detailed Steps (Now using the SectionStats Display trait) ---
        writeln!(f, "\n\n--- Detailed Processing Steps ⏱️ ---")?;
        if self.sections.is_empty() {
            writeln!(f, "    (No detailed sections recorded)")?;
        } else {
            for section in &self.sections {
                writeln!(f, "    - {}", section)?;
            }
        }

        Ok(())
    }
}
