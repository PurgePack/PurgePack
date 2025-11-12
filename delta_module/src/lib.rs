use std::{
    fs::File,
    io::{self, BufRead, Read, Write},
    path::{self},
};

use shared_files::core_header::{self};

#[derive(Debug, Clone, Copy)]
enum Transform {
    Encode,
    Decode,
}

mod cli_parse;
#[unsafe(no_mangle)]
extern "system" fn module_startup(_core: &core_header::CoreH) {
    match cli_parse::parse_args() {
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

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(_core: &core_header::CoreH) {
    println!("Goodbye world!");
}

fn start_proccessing_file(
    input_file: path::PathBuf,
    output_file: path::PathBuf,
    transform_type: Transform,
    _stats: bool,
) -> Result<(), io::Error> {
    let input = File::open(input_file)?;
    let output = File::create(output_file)?;
    let mut buff_reader = std::io::BufReader::new(input);
    let mut buff_writer = std::io::BufWriter::new(output);
    let mut previes_byte: u8;

    previes_byte = match set_delta_seed(&mut buff_reader, &mut buff_writer) {
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
        previes_byte = transform_data_chunk(
            current_chunk,
            &mut buff_writer,
            previes_byte,
            transform_type,
        )?;
        buff_reader.consume(chunk_length);
    }

    buff_writer.flush()?;
    Ok(())
}

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
