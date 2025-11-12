//! A simple canonical Huffman-coding compressor/decompressor.
//!
//! This module reads a file, computes byte frequencies, builds a Huffman tree,
//! generates canonical codes, compresses the data, writes it to a file, then
//! reads it back and verifies correctness. It uses `BitWriter` and
//! `BitReader` to operate bit-wise on buffers.

use shared_files::core_header::{self, ping_core};
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::File,
    io::{self, Read, Write},
    time::Instant,
};

/// A helper structure for writing bits into a buffer, then flushing to a file.
struct BitWriter {
    buffer: Vec<u8>,
    current_byte: u8,
    bit_pos: u8,
}

impl BitWriter {
    /// Creates a new `BitWriter` with an empty buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut writer = BitWriter::new();
    /// writer.write_bit(1);
    /// writer.write_bit(0);
    /// writer.flush();
    /// writer.flush_to_file("out.bin");
    /// ```
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            current_byte: 0,
            bit_pos: 0,
        }
    }

    /// Writes a single bit (0 or 1) into the buffer.
    ///
    /// If the bit position reaches 8, the current byte is pushed into the buffer and a new
    /// byte is started.
    ///
    /// # Panics
    ///
    /// This method does **not** panic for invalid bit values; it treats any non-zero value as 1.
    pub fn write_bit(&mut self, bit: u8) {
        if bit != 0 {
            self.current_byte |= 1 << (7 - self.bit_pos);
        }
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
            self.bit_pos = 0;
        }
    }

    /// Writes a slice of bits (each element 0 or 1) into the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut writer = BitWriter::new();
    /// writer.write_bits(&[1,0,1,1,0]);
    /// writer.flush();
    /// ```
    pub fn write_bits(&mut self, bits: &[u8]) {
        for &b in bits {
            self.write_bit(b);
        }
    }

    /// Flushes any remaining bits (less than a full byte) into the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut writer = BitWriter::new();
    /// writer.write_bit(1);
    /// writer.flush();
    /// // the buffer now contains one byte with the bit in the MSB position
    /// ```
    pub fn flush(&mut self) {
        if self.bit_pos > 0 {
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
            self.bit_pos = 0;
        }
    }

    /// Flushes the buffer to a file at the given `path`.
    ///
    /// # Panics
    ///
    /// Panics if writing to the file fails.
    pub fn flush_to_file(&mut self, path: &str) {
        self.flush();
        std::fs::write(path, &self.buffer).expect("Failed to write file");
    }
}

/// A helper structure for reading individual bits from a file into memory.
struct BitReader {
    buffer: Vec<u8>,
    byte_pos: usize,
    bit_pos: u8,
}

impl BitReader {
    /// Creates a new `BitReader` with no data loaded.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut reader = BitReader::new();
    /// reader.load_from_file("out.bin").unwrap();
    /// ```
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Loads the entire file at `path` into the internal buffer.
    ///
    /// # Errors
    ///
    /// Returns an `io::Error` if reading the file fails.
    pub fn load_from_file(&mut self, path: &str) -> io::Result<()> {
        self.buffer = std::fs::read(path)?;
        self.byte_pos = 0;
        self.bit_pos = 0;
        Ok(())
    }

    /// Reads the next bit from the buffer, returning `Some(0)` or `Some(1)`, or `None`
    /// if end-of-buffer has been reached.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut reader = BitReader::new();
    /// reader.load_from_file("out.bin").unwrap();
    /// if let Some(bit) = reader.read_bit() {
    ///     println!("Read bit: {}", bit);
    /// }
    /// ```
    pub fn read_bit(&mut self) -> Option<u8> {
        if self.byte_pos >= self.buffer.len() {
            return None;
        }
        let bit = (self.buffer[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(bit)
    }
}

/// A node in the decoding tree used for canonical Huffman decoding.
#[derive(Debug)]
struct DecodeNode {
    left: Option<Box<DecodeNode>>,
    right: Option<Box<DecodeNode>>,
    byte: Option<u8>,
}

impl DecodeNode {
    /// Creates a new empty `DecodeNode`.
    ///
    /// # Examples
    ///
    /// ```
    /// let node = DecodeNode::new();
    /// ```
    pub fn new() -> Self {
        DecodeNode {
            left: None,
            right: None,
            byte: None,
        }
    }

    /// Inserts a (bit-code, byte) pair into the decoding tree.
    ///
    /// * `code` is a slice of bits (`0` or `1`) representing the path from the root:
    ///   `0` means go left, `1` means go right.
    /// * `byte` is the value stored at the leaf corresponding to that code.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut root = DecodeNode::new();
    /// root.insert(&[0,1,0], 42u8);
    /// ```
    pub fn insert(&mut self, code: &[u8], byte: u8) {
        let mut node = self;
        for &bit in code {
            node = if bit == 0 {
                node.left.get_or_insert_with(|| Box::new(DecodeNode::new()))
            } else {
                node.right.get_or_insert_with(|| Box::new(DecodeNode::new()))
            };
        }
        node.byte = Some(byte);
    }
}

/// Builds a decoding tree from an array of optional codes for each byte value.
///
/// * `codes` is an array of length 256 (one entry per possible `u8` value),
///   where each `Option<Vec<u8>>` is the bit-code assigned to that byte (or `None` if unused).
///
/// # Examples
///
/// ```
/// let codes: [Option<Vec<u8>>; 256] = /* … */ std::array::from_fn(|_| None);
/// let tree = build_decoding_tree(&codes);
/// ```
fn build_decoding_tree(codes: &[Option<Vec<u8>>; 256]) -> DecodeNode {
    let mut root = DecodeNode::new();

    for (byte, code_opt) in codes.iter().enumerate() {
        if let Some(code) = code_opt {
            root.insert(code, byte as u8);
        }
    }

    root
}

/// Decodes a sequence of bits (0/1) using the provided decoding tree.
/// Returns the decoded bytes in a `Vec<u8>`.
///
/// # Examples
///
/// ```
/// let codes: [Option<Vec<u8>>; 256] = /* from canonical codes */;
/// let tree = build_decoding_tree(&codes);
/// let decoded = decode_canonical(&[0,1,1,0, …], &tree);
/// ```
fn decode_canonical(bits: &[u8], root: &DecodeNode) -> Vec<u8> {
    let mut result = Vec::new();
    let mut node = root;

    for &bit in bits {
        node = if bit == 0 {
            node.left.as_ref().unwrap()
        } else {
            node.right.as_ref().unwrap()
        };

        if let Some(b) = node.byte {
            result.push(b);
            node = root;
        }
    }

    result
}

/// A node used to build the Huffman tree for frequency encoding.
#[derive(Debug, Eq)]
struct Node {
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
    num: Option<u32>,
    byte: Option<u8>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.num == other.num
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.num.unwrap().cmp(&other.num.unwrap())
    }
}

/// Calculates the frequency of each possible byte value in the given buffer.
/// Returns a `[u32; 256]` array where element i counts occurrences of byte i.
///
/// # Examples
///
/// ```
/// let buffer = vec![0u8, 255u8, 0u8];
/// let freqs = calculate_byte_frequencies(&buffer);
/// assert_eq!(freqs[0], 2);
/// assert_eq!(freqs[255], 1);
/// ```
fn calculate_byte_frequencies(buffer: &Vec<u8>) -> [u32; 256] {
    let mut frequencies = [0u32; 256];
    for &byte in buffer.iter() {
        frequencies[byte as usize] += 1;
    }
    frequencies
}

/// Builds the Huffman tree from the given frequency_counts array.
///
/// Returns the root node of the Huffman tree.
///
/// # Examples
///
/// ```
/// let freqs = calculate_byte_frequencies(&vec![1u8,2u8,2u8]);
/// let root = generate_huffman_tree(&freqs);
/// ```
fn generate_huffman_tree(frequencies: &[u32; 256]) -> Box<Node> {
    let mut heap = BinaryHeap::new();

    for (byte, &freq) in frequencies.iter().enumerate() {
        if freq > 0 {
            heap.push(Reverse(Box::new(Node {
                left: None,
                right: None,
                num: Some(freq),
                byte: Some(byte as u8),
            })));
        }
    }

    while heap.len() > 1 {
        let node1 = heap.pop().unwrap();
        let node2 = heap.pop().unwrap();

        heap.push(Reverse(Box::new(Node {
            num: Some(node1.0.num.unwrap() + node2.0.num.unwrap()),
            left: Some(node1.0),
            right: Some(node2.0),
            byte: None,
        })));
    }

    heap.pop().unwrap().0
}

/// Traverses the Huffman tree to generate bit-codes (Vec<u8> of 0/1) for each byte value.
/// Returns a `Vec<Vec<u8>>` of length 256, where entry i is the code for byte i (empty if unused).
///
/// # Examples
///
/// ```
/// let root = generate_huffman_tree(&freqs);
/// let codes = generate_byte_codes(&root);
/// ```
fn generate_byte_codes(root: &Node) -> Vec<Vec<u8>> {
    let mut codes = vec![Vec::new(); 256];

    fn traverse(node: &Node, current: Vec<u8>, codes: &mut Vec<Vec<u8>>) {
        if let Some(b) = node.byte {
            codes[b as usize] = current;
            return;
        }

        if let Some(ref left) = node.left {
            let mut left_code = current.clone();
            left_code.push(0);
            traverse(left, left_code, codes);
        }

        if let Some(ref right) = node.right {
            let mut right_code = current.clone();
            right_code.push(1);
            traverse(right, right_code, codes);
        }
    }

    traverse(root, Vec::new(), &mut codes);
    codes
}

/// Converts a slice of bits (`0` or `1`) into a `Vec<u8>` of bytes (big-endian within each byte).
///
/// # Examples
///
/// ```
/// let bits = vec![1,0,1,0,0,0,0,1];
/// let bytes = bits_to_bytes(&bits);
/// assert_eq!(bytes, vec![0b10100001]);
/// ```
fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; (bits.len() + 7) / 8];
    for (i, &bit) in bits.iter().enumerate() {
        if bit != 0 {
            bytes[i / 8] |= 1 << (7 - (i % 8));
        }
    }
    bytes
}

/// Given a slice of `(byte, length)` pairs, generates canonical Huffman codes:
/// an array of 256 `Option<Vec<u8>>`, where each entry is either `None` (unused byte)
/// or `Some(code_bits)`.
///
/// # Examples
///
/// ```
/// let byte_length_pairs = vec![(0u8,3), (5u8,3), (10u8,4)];
/// let codes = generate_canonical_codes(&byte_length_pairs);
/// assert!(codes[0].is_some());
/// ```
fn generate_canonical_codes(byte_length_pairs: &[(u8, usize)]) -> [Option<Vec<u8>>; 256] {
    let mut codes: [Option<Vec<u8>>; 256] = std::array::from_fn(|_| None);

    let mut sorted = byte_length_pairs.to_vec();
    sorted.sort_by(|a, b| {
        let len_cmp = a.1.cmp(&b.1);
        if len_cmp == std::cmp::Ordering::Equal {
            a.0.cmp(&b.0)
        } else {
            len_cmp
        }
    });

    let mut current_code: u32 = 0;
    let mut prev_length: usize = 0;

    for &(byte, length) in &sorted {
        current_code <<= length - prev_length;

        let mut canonical_code = Vec::with_capacity(length);
        for i in (0..length).rev() {
            canonical_code.push(((current_code >> i) & 1) as u8);
        }

        codes[byte as usize] = Some(canonical_code);
        current_code += 1;
        prev_length = length;
    }

    codes
}

/// Compresses a buffer of bytes into a bit vector given canonical codes for each byte.
///
/// # Panics
///
/// Panics if a byte in `buffer` has no corresponding code (i.e., `byte_codes[byte]` is `None`).
///
/// # Examples
///
/// ```
/// let buffer = vec![0u8,5u8,0u8];
/// let codes = generate_canonical_codes(&[(0u8,2), (5u8,2)]);
/// let compressed = compress_canonical(&buffer, &codes);
/// ```
fn compress_canonical(buffer: &Vec<u8>, byte_codes: &[Option<Vec<u8>>; 256]) -> Vec<u8> {
    let mut compressed_bits = Vec::new();

    for &byte in buffer.iter() {
        if let Some(code) = &byte_codes[byte as usize] {
            compressed_bits.extend_from_slice(code);
        } else {
            panic!("Byte value {} has no canonical code", byte);
        }
    }

    compressed_bits
}

/// Writes canonical-encoded data to a file:
///
/// 1. Writes a 32-bit big-endian integer for the table length (# of byte/length pairs).  
/// 2. Writes a 32-bit big-endian integer for the data-length (number of bits of compressed data).  
/// 3. For each `(byte, length)` pair: writes the byte as 8 bits, then length as 8 bits.  
/// 4. Writes the compressed bit-stream.  
///
/// # Examples
///
/// ```
/// write_data_canonical(&[(0u8,2),(5u8,2)], &compressed_bits, "out.purgepack");
/// ```
fn write_data_canonical(
    byte_lengths: &[(u8, usize)],
    compressed_bits: &[u8],
    output_path: &str,
) {
    let mut writer = BitWriter::new();

    let table_len = byte_lengths.len() as u32;
    for i in (0..32).rev() {
        writer.write_bit(((table_len >> i) & 1) as u8);
    }

    let data_len = compressed_bits.len() as u32;
    for i in (0..32).rev() {
        writer.write_bit(((data_len >> i) & 1) as u8);
    }

    for &(byte, length) in byte_lengths {
        for i in (0..8).rev() {
            writer.write_bit((byte >> i) & 1);
        }
        let len_u8 = length as u8;
        for i in (0..8).rev() {
            writer.write_bit((len_u8 >> i) & 1);
        }
    }

    writer.write_bits(compressed_bits);
    writer.flush_to_file(output_path);
}

/// Reads canonical-encoded data from a file (written by `write_data_canonical`),
/// decodes it, and returns the decompressed `Vec<u8>`.
///
/// # Errors
///
/// Returns an `io::Error` if reading the file fails.
/// # Panics
///
/// Panics if bit-reading fails unexpectedly or if codes cannot be built/decoded properly.
///
/// # Examples
///
/// ```
/// let decompressed = read_data_canonical("out.purgepack").unwrap();
/// ```
fn read_data_canonical(output_path: &str) -> io::Result<Vec<u8>> {
    let mut reader = BitReader::new();
    reader.load_from_file(output_path)?;

    let mut table_len_bits = Vec::new();
    for _ in 0..32 {
        table_len_bits.push(reader.read_bit().unwrap());
    }
    let table_len = u32::from_be_bytes(bits_to_bytes(&table_len_bits).try_into().unwrap());

    let mut data_len_bits = Vec::new();
    for _ in 0..32 {
        data_len_bits.push(reader.read_bit().unwrap());
    }
    let data_len = u32::from_be_bytes(bits_to_bytes(&data_len_bits).try_into().unwrap());

    let mut byte_lengths = Vec::with_capacity(table_len as usize);
    for _ in 0..table_len {
        let mut byte_bits = Vec::new();
        for _ in 0..8 {
            byte_bits.push(reader.read_bit().unwrap());
        }
        let byte = u8::from_be_bytes(bits_to_bytes(&byte_bits).try_into().unwrap());

        let mut len_bits = Vec::new();
        for _ in 0..8 {
            len_bits.push(reader.read_bit().unwrap());
        }
        let length = u8::from_be_bytes(bits_to_bytes(&len_bits).try_into().unwrap()) as usize;

        byte_lengths.push((byte, length));
    }

    let codes: [Option<Vec<u8>>; 256] = generate_canonical_codes(&byte_lengths);

    let mut compressed_bits = Vec::with_capacity(data_len as usize);
    for _ in 0..data_len {
        compressed_bits.push(reader.read_bit().unwrap());
    }
    let decoding_root = build_decoding_tree(&codes);
    Ok(decode_canonical(&compressed_bits, &decoding_root))
}

/// Entry-point for the compressor: reads the input file (from `core.args[1]`),
/// compresses it using canonical Huffman coding, writes output, then reads back
/// to verify, and writes the decompressed result (to `core.args[3]`).
///
/// # Panics
///
/// Panics if any file I/O fails or code logic fails.
/// # Usage
///
/// This is intended to be invoked via `module_startup`.
fn canonical_huffman(core: &core_header::CoreH, args: &mut Vec<String>) {
    ping_core(&core);

    let debug_whole_timer = Instant::now();
    let mut debug_timer = Instant::now();

    let mut buffer: Vec<u8> = Vec::new();
    let mut file_to_compress;

    if args.len() != 3 {
        println!("Expected 3 arguments, got {}", args.len());
        return;
    }

    match File::open(&args[0]) {
        Ok(file) => file_to_compress = file,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        }
    }

    if let Err(msg) = file_to_compress.read_to_end(&mut buffer) {
        println!("Error: {:?}", msg);
        return;
    }
    println!("Read file: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let chars_frequency_map = calculate_byte_frequencies(&buffer);
    println!("Calculated frequency: {:.2?}", debug_timer.elapsed());

    debug_timer = Instant::now();
    let root_node = generate_huffman_tree(&chars_frequency_map);
    println!("Calculated huffman tree: {:.2?}", debug_timer.elapsed());

    debug_timer = Instant::now();
    let byte_codes = generate_byte_codes(&root_node);
    println!("Calculated byte codes: {:.2?}", debug_timer.elapsed());

    debug_timer = Instant::now();
    let code_lengths: Vec<(u8, usize)> = byte_codes
        .iter()
        .enumerate()
        .filter_map(|(b, c)| if !c.is_empty() { Some((b as u8, c.len())) } else { None })
        .collect();
    let codes = generate_canonical_codes(&code_lengths);
    println!("Calculated canonical byte codes {:.2?}", debug_timer.elapsed());

    debug_timer = Instant::now();
    let compressed_bits = compress_canonical(&buffer, &codes);
    println!("Calculated compressed bytes: {:.2?}", debug_timer.elapsed());

    debug_timer = Instant::now();
    let comp_path = args[1].clone() + "/compressed_canonical.purgepack";

    write_data_canonical(&code_lengths, &compressed_bits, &comp_path);
    println!("Wrote data: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let back_buffer;
    match read_data_canonical(&comp_path) {
        Ok(data) => back_buffer = data,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        }
    }
    println!("Read data: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    println!("Does the decompressed file matching?: {}", buffer == back_buffer);

    let res_path = args[2].clone();
    let mut result;
    match File::create(res_path) {
        Ok(data) => result = data,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        }
    }

    if let Err(msg) = result.write(&back_buffer) {
        println!("Error: {:?}", msg);
        return;
    }
    println!("Written read data: {:.2?}", debug_timer.elapsed());

    let compressed_file;
    match File::open(comp_path) {
        Ok(file) => compressed_file = file,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        }
    }

    println!("Elapsed: {:.2?}", debug_whole_timer.elapsed());
    println!("Original size: {} bytes", buffer.len());
    println!("Compressed size: {} bits", compressed_bits.len());
    println!(
        "Compressed size compared to original: {}%",
        (compressed_file.metadata().unwrap().len() as f32 / buffer.len() as f32) * 100.0
    );
}

/// Called when the module starts up: invokes `canonical_huffman`.
#[unsafe(no_mangle)]
extern "C" fn module_startup(core: &core_header::CoreH, args: &mut Vec<String>) {
    canonical_huffman(core, args);
}

/// Called when the module is shutting down.
#[unsafe(no_mangle)]
extern "C" fn module_shutdown(_core: &core_header::CoreH) {}
