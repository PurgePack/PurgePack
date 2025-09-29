use shared_files::core_header::{self};
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::File,
    io::{self, Read, Write},
    time::Instant,
};

struct SimpleBitWriter {
    buffer: Vec<u8>,
    current_byte: u8,
    bit_pos: u8,
}

impl SimpleBitWriter {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            current_byte: 0,
            bit_pos: 0,
        }
    }

    fn write_bit(&mut self, bit: u8) {
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

    fn write_bits(&mut self, bits: &[u8]) {
        for &b in bits {
            self.write_bit(b);
        }
    }

    fn flush(&mut self) {
        if self.bit_pos > 0 {
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
            self.bit_pos = 0;
        }
    }

    fn flush_to_file(&mut self, path: &str) {
        self.flush();
        std::fs::write(path, &self.buffer).expect("Failed to write file");
    }
}

struct SimpleBitReader {
    buffer: Vec<u8>,
    byte_pos: usize,
    bit_pos: u8,
}

impl SimpleBitReader {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn load_from_file(&mut self, path: &str) -> io::Result<()> {
        self.buffer = std::fs::read(path)?;
        self.byte_pos = 0;
        self.bit_pos = 0;
        Ok(())
    }

    fn read_bit(&mut self) -> Option<u8> {
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

#[derive(Debug)]
struct DecodeNode {
    left: Option<Box<DecodeNode>>,
    right: Option<Box<DecodeNode>>,
    byte: Option<u8>,
}

impl DecodeNode {
    fn new() -> Self {
        DecodeNode {
            left: None,
            right: None,
            byte: None,
        }
    }

    fn insert(&mut self, code: &[u8], byte: u8) {
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

fn build_decoding_tree(codes: &[Option<Vec<u8>>; 256]) -> DecodeNode {
    let mut root = DecodeNode::new();

    for (byte, code_opt) in codes.iter().enumerate() {
        if let Some(code) = code_opt {
            root.insert(code, byte as u8);
        }
    }

    root
}

fn decode_canonical(reader: &mut SimpleBitReader, root: &DecodeNode, data_len: u32) -> Vec<u8> {
    let mut result = Vec::with_capacity(data_len as usize);
    let mut node = root;

    while let Some(bit) = reader.read_bit() {
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

fn calculate_byte_frequencies(buffer: &[u8]) -> [u32; 256] {
    let mut frequencies = [0u32; 256];
    for &byte in buffer.iter() {
        frequencies[byte as usize] += 1;
    }
    frequencies
}

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

fn compress_canonical(buffer: &[u8], codes: &[Option<Vec<u8>>; 256], writer: &mut SimpleBitWriter) {
    for &byte in buffer.iter() {
        if let Some(code) = &codes[byte as usize] {
            writer.write_bits(code);
        }
    }
}

fn write_data_canonical(
    byte_lengths: &[(u8, usize)],
    buffer: &[u8],
    codes: &[Option<Vec<u8>>; 256],
    output_path: &str,
) {
    let mut writer = SimpleBitWriter::new();

    let table_len = byte_lengths.len() as u32;
    for i in (0..32).rev() {
        writer.write_bit(((table_len >> i) & 1) as u8);
    }

    let data_len = buffer.len() as u32;
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

    compress_canonical(buffer, codes, &mut writer);
    writer.flush_to_file(output_path);
}

fn read_data_canonical(output_path: &str) -> io::Result<Vec<u8>> {
    let mut reader = SimpleBitReader::new();
    reader.load_from_file(output_path)?;

    let mut table_len = 0u32;
    for _ in 0..32 {
        table_len = (table_len << 1) | reader.read_bit().unwrap() as u32;
    }

    let mut data_len = 0u32;
    for _ in 0..32 {
        data_len = (data_len << 1) | reader.read_bit().unwrap() as u32;
    }

    let mut byte_lengths = Vec::with_capacity(table_len as usize);
    for _ in 0..table_len {
        let mut byte = 0u8;
        for _ in 0..8 {
            byte = (byte << 1) | reader.read_bit().unwrap();
        }
        let mut length = 0u8;
        for _ in 0..8 {
            length = (length << 1) | reader.read_bit().unwrap();
        }
        byte_lengths.push((byte, length as usize));
    }

    let codes = generate_canonical_codes(&byte_lengths);
    let root = build_decoding_tree(&codes);

    Ok(decode_canonical(&mut reader, &root, data_len))
}

fn canonical_huffman(core: &core_header::CoreH) {
    let debug_whole_timer = Instant::now();
    let mut debug_timer = Instant::now();

    let mut buffer = Vec::new();
    let mut file_to_compress = File::open(&core.args[1]).expect("Failed to open file");
    file_to_compress.read_to_end(&mut buffer).expect("Failed to read file");

    println!("Read file: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let frequencies = calculate_byte_frequencies(&buffer);
    let root_node = generate_huffman_tree(&frequencies);
    let byte_codes = generate_byte_codes(&root_node);

    println!("Generated Huffman tree & codes: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let code_lengths: Vec<(u8, usize)> = byte_codes
        .iter()
        .enumerate()
        .filter_map(|(b, c)| if !c.is_empty() { Some((b as u8, c.len())) } else { None })
        .collect();

    let codes = generate_canonical_codes(&code_lengths);

    let compressed_path = core.args[2].to_str().unwrap().to_owned() + "/compressed_canonical.purgepack";
    write_data_canonical(&code_lengths, &buffer, &codes, &compressed_path);

    println!("Compressed data written: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let back_buffer = read_data_canonical(&compressed_path).expect("Failed to read compressed data");

    println!("Decompressed matches original: {}", buffer == back_buffer);
    println!("Read & decompressed: {:.2?}", debug_timer.elapsed());

    let mut result_file = File::create(&core.args[3]).expect("Failed to create result file");
    result_file.write_all(&back_buffer).expect("Failed to write decompressed data");

    println!("Elapsed: {:.2?}", debug_whole_timer.elapsed());
}

#[unsafe(no_mangle)]
extern "system" fn module_startup(core: &core_header::CoreH) {
    canonical_huffman(core);
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &mut core_header::CoreH, _exiting: bool) {

}
