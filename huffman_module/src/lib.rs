use shared_files::core_header::{self};
use std::{
    cmp::Reverse, collections::BinaryHeap, fs::File, io::{self, Read, Write}, time::Instant
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
        if bit != 0 { self.current_byte |= 1 << (7 - self.bit_pos); }
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
            self.bit_pos = 0;
        }
    }

    fn write_bits(&mut self, bits: &[u8]) {
        for &b in bits { self.write_bit(b); }
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
        Self { buffer: Vec::new(), byte_pos: 0, bit_pos: 0 }
    }

    fn load_from_file(&mut self, path: &str) -> io::Result<()> {
        self.buffer = std::fs::read(path)?;
        self.byte_pos = 0;
        self.bit_pos = 0;
        Ok(())
    }

    fn read_bit(&mut self) -> Option<u8> {
        if self.byte_pos >= self.buffer.len() { return None; }
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
        DecodeNode { left: None, right: None, byte: None }
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

fn decode_canonical(bits: &[u8], root: &DecodeNode) -> Vec<u8> {
    let mut result = Vec::new();
    let mut node = root;

    for &bit in bits {
        node = if bit == 0 { node.left.as_ref().unwrap() } else { node.right.as_ref().unwrap() };

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

fn calculate_byte_frequencies(buffer: &Vec<u8>) -> [u32; 256] {
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
            heap.push(
                Reverse(Box::new(Node {
                    left: None,
                    right: None,
                    num: Some(freq),
                    byte: Some(byte as u8),
                })),
            );
        }
    }

    while heap.len() > 1 {
        let node1 = heap.pop().unwrap();
        let node2 = heap.pop().unwrap();

        heap.push(
            Reverse(Box::new(Node {
                num: Some(node1.0.num.unwrap() + node2.0.num.unwrap()),
                left: Some(node1.0),
                right: Some(node2.0),
                byte: None,
            })),
        );
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

fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; (bits.len() + 7) / 8];
    for (i, &bit) in bits.iter().enumerate() {
        if bit != 0 { bytes[i / 8] |= 1 << (7 - (i % 8)); }
    }
    bytes
}

fn generate_canonical_codes(byte_length_pairs: &[(u8, usize)]) -> [Option<Vec<u8>>; 256] {
    let mut codes: [Option<Vec<u8>>; 256] = std::array::from_fn(|_| None);

    let mut sorted = byte_length_pairs.to_vec();
    sorted.sort_by(|a, b| {
        let len_cmp = a.1.cmp(&b.1);
        if len_cmp == std::cmp::Ordering::Equal { a.0.cmp(&b.0) } else { len_cmp }
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

fn compress_canonical(buffer: &Vec<u8>, byte_codes: &[Option<Vec<u8>>; 256]) -> Vec<u8> {
    let mut compressed_bits = Vec::new();

    for &byte in buffer.iter() {
        if let Some(code) = &byte_codes[byte as usize] {
            compressed_bits.extend_from_slice(code);
        }
    }

    compressed_bits
}

fn write_data_canonical(
    byte_lengths: &[(u8, usize)],
    compressed_bits: &[u8],
    output_path: &str,
) {
    let mut writer = SimpleBitWriter::new();

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

fn read_data_canonical(output_path: &str) -> io::Result<Vec<u8>> {
    let mut reader = SimpleBitReader::new();
    reader.load_from_file(output_path)?;

    let mut table_len_bits = Vec::new();
    for _ in 0..32 { table_len_bits.push(reader.read_bit().unwrap()); }
    let table_len = u32::from_be_bytes(bits_to_bytes(&table_len_bits).try_into().unwrap());

    let mut data_len_bits = Vec::new();
    for _ in 0..32 { data_len_bits.push(reader.read_bit().unwrap()); }
    let data_len = u32::from_be_bytes(bits_to_bytes(&data_len_bits).try_into().unwrap());

    let mut byte_lengths = Vec::with_capacity(table_len as usize);
    for _ in 0..table_len {
        let mut byte_bits = Vec::new();
        for _ in 0..8 { byte_bits.push(reader.read_bit().unwrap()); }
        let byte = u8::from_be_bytes(bits_to_bytes(&byte_bits).try_into().unwrap());

        let mut len_bits = Vec::new();
        for _ in 0..8 { len_bits.push(reader.read_bit().unwrap()); }
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

fn canonical_huffman(core: &core_header::CoreH) {
    let debug_whole_timer = Instant::now();
    let mut debug_timer = Instant::now();

    let mut buffer: Vec<u8> = Vec::new();
    let mut file_to_compress;

    match File::open(core.args[1].clone()) {
        Ok(file) => file_to_compress = file,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        },
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
    let code_lengths: Vec<(u8, usize)> = byte_codes.iter().enumerate()
        .filter_map(|(b, c)| if !c.is_empty() { Some((b as u8, c.len())) } else { None })
        .collect();
    let codes = generate_canonical_codes(&code_lengths);
    println!("Calculated canonical byte codes {:.2?}", debug_timer.elapsed());

    debug_timer = Instant::now();
    let compressed_bits = compress_canonical(&buffer, &codes);
    println!("Calculated compressed bytes: {:.2?}", debug_timer.elapsed());

    debug_timer = Instant::now();
    let mut comp_path = core.args[2].clone();
    comp_path.push("/compressed_canonical.purgepack");

    write_data_canonical(&code_lengths, &compressed_bits, comp_path.to_str().unwrap());
    println!("Wrote data: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let back_buffer;
    match read_data_canonical(comp_path.to_str().unwrap()) {
        Ok(data) => back_buffer = data,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        },
    }
    println!("Read data: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    println!("{:?}", buffer == back_buffer);

    let res_path = core.args[3].clone();
    let mut result;
    match File::create(res_path) {
        Ok(data) => result = data,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        },
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
        },
    }

    println!("Elapsed: {:.2?}", debug_whole_timer.elapsed());
    println!("Original size: {} bytes", buffer.len());
    println!("Compressed size: {} bits", compressed_bits.len());
    println!(
        "Compressed size compared to original: {}%",
        (compressed_file.metadata().unwrap().len() as f32 / buffer.len() as f32) * 100.0
    );
}

#[unsafe(no_mangle)]
extern "system" fn module_startup(core: &core_header::CoreH) {
    canonical_huffman(core);
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &mut core_header::CoreH, _exiting: bool) {

}
