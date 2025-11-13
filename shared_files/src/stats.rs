use std::fmt::{self, Display};
use std::time::Duration;

/// A simple helper function to format bytes into human-readable strings.
fn format_bytes(bytes: usize) -> String {
    const KIB: usize = 1024;
    const MIB: usize = KIB * 1024;
    const GIB: usize = MIB * 1024;

    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} Bytes", bytes)
    }
}

/// A struct to hold all necessary inputs and calculated outputs for a
/// compression or decompression run.
pub struct CompressionStats {
    pub algorithm_name: &'static str,
    pub algorithm_id: u8,
    pub version_used: u8,
    pub original_len: usize,
    pub processed_len: usize,
    pub duration: Duration,
    pub is_compression: bool,

    // Calculated fields
    pub compression_ratio_factor: f64,
    pub speed_mib_s: f64,
    pub raw_byte_difference: i64,
    pub percentage_change: f64,
}

impl CompressionStats {
    /// Constructor for the CompressionStats struct.
    /// # Arguments
    /// * `algorithm_name` - The name of the compression algorithm used.
    /// * `algorithm_id` - The ID of the compression algorithm used.
    /// * `version_used` - The version of the compression algorithm used.
    /// * `original_len` - The original size of the input data in bytes.
    /// * `processed_len` - The size of the processed data in bytes.
    /// * `duration` - The duration of the compression or decompression process.
    /// * `is_compression` - A boolean flag indicating whether the process is a compression or decompression operation.
    /// # Returns
    /// * `CompressionStats` - A new instance of the CompressionStats struct.
    /// # Examples
    /// ```rust
    /// use shared_files::stats::CompressionStats;
    /// let stats = CompressionStats::calculate_stats("LZ4", 1, 1, 1024, 512, Duration::from_secs(1), true);
    /// ```
    pub fn calculate_stats(
        algorithm_name: &'static str,
        algorithm_id: u8,
        version_used: u8,
        original_len: usize,
        processed_len: usize,
        duration: Duration,
        is_compression: bool,
    ) -> Self {
        let (uncompressed_len, compressed_len) = if is_compression {
            (original_len, processed_len)
        } else {
            (processed_len, original_len)
        };
        let compression_ratio_factor = uncompressed_len as f64 / compressed_len as f64;
        let speed_mib_s = (uncompressed_len as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();
        let raw_byte_difference = uncompressed_len as i64 - compressed_len as i64;
        let difference_bytes = raw_byte_difference.abs() as usize;
        let percentage_base = uncompressed_len as f64;
        let percentage_change = (difference_bytes as f64 / percentage_base) * 100.0;
        CompressionStats {
            algorithm_name,
            algorithm_id,
            version_used,
            original_len,
            processed_len,
            duration,
            is_compression,
            compression_ratio_factor,
            speed_mib_s,
            raw_byte_difference,
            percentage_change,
        }
    }
}

// Implement the Display trait to define exactly how the statistics struct is printed.
impl Display for CompressionStats {
    // Define the formatting logic for the CompressionStats struct.
    // # Arguments
    // * `f` - A mutable reference to the formatter object.
    // # Returns
    // * `fmt::Result` - The result of the formatting operation.
    // # Examples
    // ```rust
    // use shared_files::stats::CompressionStats;
    // let stats = CompressionStats::calculate_stats("LZ4", 1, 1, 1024, 512, Duration::from_secs(1), true);
    // println!("{}", stats);
    // ```
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

        Ok(())
    }
}
