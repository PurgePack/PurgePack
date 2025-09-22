use bit_buffers::{BitReader, BitWriter};
use indexmap::IndexMap;
use shared_files::core_header::{self};
use std::{
    fs::File, io::{Read, Write}, rc::Rc, time::{Instant}
};

#[derive(Debug)]
struct Node {
    left: Option<Rc<Node>>,
    right: Option<Rc<Node>>,
    num: Option<u32>,
    byte: Option<u8>,
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

fn generate_huffman_tree(chars_frequency_map: &IndexMap<u8, u32>) -> Rc<Node> {
    let mut huffman_tree: Vec<Rc<Node>> = Vec::new();

    for i in chars_frequency_map.iter() {
        huffman_tree.push(
            Rc::new(
                Node {
                    left: (None),
                    right: (None),
                    num: Some(i.1.clone()),
                    byte: Some(i.0.clone()),
                }
            )
        );
    }

    while huffman_tree.len() > 1 {
        huffman_tree.sort_by(|a, b| b.num.unwrap().cmp(&a.num.unwrap()));

        let node1 = huffman_tree.pop().unwrap();
        let node2 = huffman_tree.pop().unwrap();

        huffman_tree.insert(
            0,
            Rc::new(
                Node {
                    left: Some(node1.clone()),
                    right: Some(node2.clone()),
                    num: Some(node1.num.unwrap() + node2.num.unwrap()),
                    byte: None,
                }
            ),
        );
    }

    huffman_tree.first().unwrap().clone()
}

fn generate_char_codes(root: Rc<Node>) -> Vec<(u8, Vec<u8>)> {
    let mut codes = Vec::new();

    generate_char_codes_internal(
        root.left.as_ref().unwrap().clone(),
        vec![0],
        &mut codes,
    );

    generate_char_codes_internal(
        root.right.as_ref().unwrap().clone(),
        vec![1],
        &mut codes,
    );

    codes
}

fn generate_char_codes_internal(
    root: Rc<Node>,
    mut current_code: Vec<u8>,
    codes: &mut Vec<(u8, Vec<u8>)>,
) {
    if root.byte != None {
        codes.push((root.byte.unwrap(), current_code.clone()));
        return;
    }

    current_code.push(0);
    generate_char_codes_internal(root.left.as_ref().unwrap().clone(), current_code.clone(), codes);

    current_code.pop();
    current_code.push(1);
    generate_char_codes_internal(root.right.as_ref().unwrap().clone(), current_code.clone(), codes);
}

#[unsafe(no_mangle)]
extern "system" fn module_startup(core: &core_header::CoreH) {
    let debug_timer = Instant::now();

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

    let chars_frequency_map = calculate_byte_frequencies(&buffer);
    let huffman_tree = generate_huffman_tree(&chars_frequency_map);

    let mut debug_file;
    let mut debug_file_path = core.args[2].clone();
    debug_file_path.push("/debug.txt");

    match File::create(debug_file_path) {
        Ok(data) => debug_file = data,
        Err(msg) => {
            println!("Error: {:?}", msg);
            return;
        },
    }
    
    if let Err(msg) = debug_file.write(format!("{:#?}", huffman_tree).as_bytes()) {
        println!("Error: {:?}", msg);
    }

    // get char codes

    let char_codes = generate_char_codes(huffman_tree);

    // compress string

    let mut compressed_str: Vec<Vec<u8>> = Vec::new();
    let mut compressed_str_string = String::new();

    for byte in buffer.iter() {
        for code in char_codes.iter() {
            if code.0 == *byte {
                compressed_str.push(code.1.clone());
                for ch in code.1.iter() {
                    compressed_str_string.push_str(&ch.to_string());
                }
                break;
            }
        }
    }

    // prepare writing

    let mut writer = BitWriter::new();

    // header
    let code_table_length: u32 = char_codes.len() as u32;
    let data_bit_length: u32 = compressed_str_string.len() as u32;

    // table

    let mut table_bytes: Vec<(String, String)> = Vec::new();

    for code in char_codes.iter() {
        let slice = format!("{:08b}", code.0);

        let mut code_bytes = String::new();

        for bit in code.1.iter() {
            code_bytes.push_str(&bit.to_string());
        }

        table_bytes.push((slice, code_bytes));
    }

    // write header

    // code table length

    for charbit in format!("{:032b}", code_table_length).chars() {
        writer.write_bit(charbit.to_digit(10).unwrap() as u8);
    }

    // data length

    for charbit in format!("{:032b}", data_bit_length).chars() {
        writer.write_bit(charbit.to_digit(10).unwrap() as u8);
    }

    // write table

    for table_element in table_bytes.iter() {
        // 1 byte

        for charbit in table_element.0.chars() {
            writer.write_bit(charbit.to_digit(10).unwrap() as u8);
        }

        // code length

        let code_length: u32 = table_element.1.len() as u32;
        for charbit in format!("{:032b}", code_length).chars() {
            writer.write_bit(charbit.to_digit(10).unwrap() as u8);
        }

        // code

        for charbit in table_element.1.chars() {
            writer.write_bit(charbit.to_digit(10).unwrap() as u8);
        }
    }

    // write data

    for byte in compressed_str.iter() {
        for bit in byte.iter() {
            writer.write_bit(bit.clone());
        }
    }

    let mut comp_path = core.args[2].clone();
    comp_path.push("/compressed.purgepack");

    writer.flush_to_file(&comp_path.to_str().unwrap());

    // read back
    let mut reader = BitReader::new();

    reader.load_from_file(&comp_path.to_str().unwrap());

    // buffer

    let mut bits = String::new();

    // read code table length

    for _i in 0..32 {
        bits.push_str(&reader.read_bit().unwrap().to_string());
    }

    let code_length = u32::from_str_radix(&bits, 2).unwrap();

    bits.clear();

    // read data length

    for _i in 0..32 {
        bits.push_str(&reader.read_bit().unwrap().to_string());
    }

    let data_length = u32::from_str_radix(&bits, 2).unwrap();

    bits.clear();

    // read char codes table
    let mut char_codes_read: Vec<(u8, Vec<u8>)> = Vec::new();

    for _i in 0..code_length {
        for _i in 0..8 {
            bits.push_str(&reader.read_bit().unwrap().to_string());
        }

        let ind_bits: String = bits.clone();
        bits.clear();

        for _i in 0..32 {
            bits.push_str(&reader.read_bit().unwrap().to_string());
        }

        let code_len = u32::from_str_radix(&bits, 2).unwrap();

        bits.clear();

        for _i in 0..code_len {
            bits.push_str(&reader.read_bit().unwrap().to_string());
        }

        let code_bits: String = bits.clone();

        let mut code_v = Vec::new();

        for chara in code_bits.chars() {
            code_v.push(chara.to_digit(10).unwrap() as u8);
        }

        char_codes_read.push((u8::from_str_radix(&ind_bits, 2).unwrap() ,code_v));
        bits.clear();
    }

    bits.clear();

    // read data

    for _i in 0..data_length {
        bits.push_str(&reader.read_bit().unwrap().to_string());
    }

    let mut back_buffer: Vec<u8> = Vec::new();
    let mut check_code_read: Vec<u8> = Vec::new();

    for bit in bits.chars() {
        check_code_read.push(bit.to_digit(10).unwrap() as u8);

        for code in char_codes_read.iter() {
            if check_code_read == code.1 {
                back_buffer.push(code.0.clone());
                check_code_read.clear();
                break;
            }
        }
    }

    println!("{:?}", buffer == back_buffer);

    let res_path = core.args[3].clone();

    let mut result = File::create(res_path).unwrap();
    result.write(&back_buffer);

    println!("Elapsed: {:.2?}", debug_timer.elapsed());
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &mut core_header::CoreH, _exiting: bool) {

}
