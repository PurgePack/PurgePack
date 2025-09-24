use bit_buffers::{BitReader, BitWriter};
use indexmap::IndexMap;
use shared_files::core_header::{self};
use std::{
    cmp::Reverse, collections::BinaryHeap, fs::File, io::{self, Read, Write}, time::Instant
};

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

fn calculate_byte_frequencies(b: &Vec<u8>) -> IndexMap<u8, u32> {
    let mut chars_frequency_map: IndexMap<u8, u32> = IndexMap::new();

    for byte in b.iter() {
        if !chars_frequency_map.contains_key(byte) {
            chars_frequency_map.insert(*byte, 1);
        }
        else {
            *chars_frequency_map.get_mut(byte).unwrap() += 1;
        }
    }

    chars_frequency_map.sort_by(|_a, b, _c, d| b.cmp(d));

    chars_frequency_map
}

fn generate_huffman_tree(bytes_frequency_map: &IndexMap<u8, u32>) -> Box<Node> {
    let mut huffman_tree = BinaryHeap::new();

    for (byte, frequency) in bytes_frequency_map.iter() {
        huffman_tree.push(
            Reverse(
                Box::new(
                    Node {
                        left: (None),
                        right: (None),
                        num: Some(*frequency),
                        byte: Some(*byte),
                    }
                )
            )
        );
    }

    while huffman_tree.len() > 1 {
        let node1 = huffman_tree.pop().unwrap();
        let node2 = huffman_tree.pop().unwrap();

        huffman_tree.push(
            Reverse(
                Box::new(
                    Node {
                        num: Some(node1.0.num.unwrap() + node2.0.num.unwrap()),
                        left: Some(node1.0),
                        right: Some(node2.0),
                        byte: None,
                    }
                )
            )
        );
    }

    huffman_tree.pop().unwrap().0
}

fn generate_byte_codes(root: &Node) -> IndexMap<u8, Vec<u8>> {
    let mut codes = IndexMap::new();

    generate_char_codes_internal(
        root.left.as_ref().unwrap(),
        vec![0],
        &mut codes,
    );

    generate_char_codes_internal(
        root.right.as_ref().unwrap(),
        vec![1],
        &mut codes,
    );

    codes
}

fn generate_char_codes_internal(
    root: &Node,
    mut current_code: Vec<u8>,
    codes: &mut IndexMap<u8, Vec<u8>>,
) {
    if root.byte != None {
        codes.insert(root.byte.unwrap(), current_code.clone());
        return;
    }

    current_code.push(0);
    generate_char_codes_internal(root.left.as_ref().unwrap(), current_code.clone(), codes);

    current_code.pop();
    current_code.push(1);
    generate_char_codes_internal(root.right.as_ref().unwrap(), current_code.clone(), codes);
}

fn compress(buffer: &Vec<u8>, byte_codes: &IndexMap<u8, Vec<u8>>) -> Vec<u8> {
    let mut compressed_bits = Vec::new();

    for byte in buffer.iter() {
        if let Some(code) = byte_codes.get(byte) {
            for bit in code {
                compressed_bits.push(*bit);
            }
        }
    }

    compressed_bits
}

fn write_data(
    byte_codes: &IndexMap<u8, Vec<u8>>,
    compressed_bytes: &Vec<u8>,
    output_path: &str
) {
    let mut writer = BitWriter::new();

    // Header.
    let code_table_length: u32 = byte_codes.len() as u32;
    let data_bit_length: u32 = compressed_bytes.len() as u32;

    // Code table length.
    for i in (0..32).rev() {
        writer.write_bit(((code_table_length >> i) & 1) as u8);
    }

    // Data length.
    for i in (0..32).rev() {
        writer.write_bit(((data_bit_length >> i) & 1) as u8);
    }

    // Write table.
    for byte_code in byte_codes.iter() {
        // Byte.
        for i in (0..8).rev() {
            writer.write_bit((*byte_code.0 >> i) & 1);
        }

        // Code length.
        let code_length: u32 = byte_code.1.len() as u32;
        for i in (0..32).rev() {
            writer.write_bit(((code_length >> i) & 1) as u8);
        }

        // Code.
        for byte in byte_code.1.iter() {
            writer.write_bit(*byte);
        }
    }

    // Write data.
    for byte in compressed_bytes.iter() {
        writer.write_bit(*byte);
    }

    writer.flush_to_file(output_path);
}

fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((bits.len() + 7) / 8);

    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit != 0 && bit != 1 {
                panic!("Invalid bit: {bit}");
            }
            byte |= bit << (7 - i); // MSB first
        }
        bytes.push(byte);
    }

    bytes
}

fn read_data(output_path: &str) -> io::Result<Vec<u8>> {
    let mut reader = BitReader::new();
    reader.load_from_file(output_path)?;

    let mut bits = Vec::new();

    // Read code table length.
    for _i in 0..32 {
        bits.push(reader.read_bit().unwrap());
    }

    let code_length = u32::from_be_bytes(bits_to_bytes(&bits).try_into().unwrap());
    bits.clear();

    // Read data length.
    for _i in 0..32 {
        bits.push(reader.read_bit().unwrap());
    }

    let data_length = u32::from_be_bytes(bits_to_bytes(&bits).try_into().unwrap());
    bits.clear();

    // Read byte codes table.
    let mut byte_codes: IndexMap<Vec<u8>, u8> = IndexMap::new();

    for _i in 0..code_length {
        // Read byte.
        for _i in 0..8 {
            bits.push(reader.read_bit().unwrap());
        }

        let byte_bits: Vec<u8> = bits.clone();
        bits.clear();

        // Read code length.
        for _i in 0..32 {
            bits.push(reader.read_bit().unwrap());
        }

        let code_len = u32::from_be_bytes(bits_to_bytes(&bits).try_into().unwrap());
        bits.clear();

        // Read code.
        for _i in 0..code_len {
            bits.push(reader.read_bit().unwrap());
        }

        let code_bits: Vec<u8> = bits.clone();

        byte_codes.insert(
            code_bits,
            u8::from_be_bytes(bits_to_bytes(&byte_bits).try_into().unwrap())
        );
        bits.clear();
    }
    bits.clear();

    // Read data.
    for _i in 0..data_length {
        bits.push(reader.read_bit().unwrap());
    }

    let mut back_buffer: Vec<u8> = Vec::new();
    let mut check_byte_read: Vec<u8> = Vec::new();

    for bit in bits.iter() {
        check_byte_read.push(*bit);

        if let Some(byte) = byte_codes.get(&check_byte_read) {
            back_buffer.push(*byte);
            check_byte_read.clear();
        }
    }

    Ok(back_buffer)
}

#[unsafe(no_mangle)]
extern "system" fn module_startup(core: &core_header::CoreH) {
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

    // let mut debug_file;
    // let mut debug_file_path = core.args[2].clone();
    // debug_file_path.push("/debug.txt");

    // match File::create(debug_file_path) {
    //     Ok(data) => debug_file = data,
    //     Err(msg) => {
    //         println!("Error: {:?}", msg);
    //         return;
    //     },
    // }
    
    // if let Err(msg) = debug_file.write(format!("{:#?}", huffman_tree).as_bytes()) {
    //     println!("Error: {:?}", msg);
    // }

    let byte_codes = generate_byte_codes(&root_node);
    println!("Calculated byte codes: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();
    let compressed_bits = compress(&buffer, &byte_codes);
    println!("Calculated compressed bytes: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let mut comp_path = core.args[2].clone();
    comp_path.push("/compressed.purgepack");

    write_data(&byte_codes, &compressed_bits, comp_path.to_str().unwrap());
    println!("Wrote data: {:.2?}", debug_timer.elapsed());
    debug_timer = Instant::now();

    let back_buffer;
    match read_data(comp_path.to_str().unwrap()) {
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
extern "system" fn module_shutdown(_core: &mut core_header::CoreH, _exiting: bool) {

}
