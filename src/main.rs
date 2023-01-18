use std::{process, env, time::Instant};

mod parse_processor;
use parse_processor::{Configuration, ParseProcessor};

pub fn print_usage() {
    println!("usage: dict_gen input_dir output_file [header_file]");
    println!("  input_dir: directory with xml files from DICOM standard");
    println!("  output_file: output file name");
    println!("  header_file: file with custom header for output_file");
}

fn main() {
    let start = Instant::now();
    let args: Vec<String> = env::args().collect();
    let config = Configuration::new(&args).unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        print_usage();
        process::exit(1);
    });
    println!("Start");
    ParseProcessor::new(config).execute().unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        process::exit(1);
    });
    println!("Finish for {} ms", start.elapsed().as_millis());
    process::exit(0);
}