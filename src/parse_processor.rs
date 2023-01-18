use std::{fs::{File, OpenOptions}, io::{BufReader, Write}};
use walkdir::WalkDir;
use quick_xml::{reader::Reader, events::Event};

pub struct Configuration {
    pub input_dir_path: String,
    pub output_file_path: String,
    pub header_file_path: String,
}

impl Configuration {
    pub fn new(args: &[String]) -> Result<Configuration, String> {
        let argc = args.len();
        if  argc < 3 {
            return Err("Not enough arguments".to_string());
        }

        Ok(Configuration {
            input_dir_path: args[1].clone(),
            output_file_path: args[2].clone(),
            header_file_path: if argc > 4 { args[3].clone() } else { String::new() }
        })
    }
}

#[derive(PartialEq, PartialOrd)]
struct DataItem {
    tag: String,
    name: String,
    keyword: String,
    vr: String,
    vm: String,
    version: String,
}

struct FindElement<'a> {
    pub start: &'a str,
    pub end: &'a str,
    pub attr: Option<(&'a str, &'a str)>,
}

pub struct ParseProcessor {
    config: Configuration,
    file_paths: Vec<String>,
    releasenotes_path: String,
    version: String,
    data_items: Vec<DataItem>,
    dictionary_header: String,
}

impl ParseProcessor {
    pub fn new(config: Configuration) -> ParseProcessor {
        let dictionary_header = if config.header_file_path.is_empty() {
            String::from_utf8_lossy(include_bytes!("dictionary_header.txt")).to_string()
        } else {
            std::fs::read_to_string(&config.header_file_path).unwrap()
        };

        Self{ 
            config, 
            file_paths: Vec::new(), 
            releasenotes_path: String::new(), 
            version: String::new(),
            data_items: Vec::new(),
            dictionary_header,
         }
    }

    pub fn execute(&mut self) -> Result<(), String> {
        print!("Read xml files ...");
        if let Err(e) = self.find_xml_files() {
            return Err(e.to_string())
        }
        print!("OK\n");
        print!("Parse dicom version ...");
        self.parse_version()?;
        print!("OK\n");
        print!("Parse xml files ...");
        self.parse_files()?;
        print!("OK\n");
        print!("Write dictionary to file ...");
        if let Err(e) = self.write_dictionary_to_file() {
            return Err(e.to_string())
        }
        print!("OK\n");
        Ok(())
    }

    fn find_xml_files(&mut self) -> Result<(), std::io::Error> {
        for entry in WalkDir::new(&self.config.input_dir_path).into_iter() {
            let dir_entry = entry?;
            let is_file = dir_entry.metadata()?.is_file();
            let file_path = dir_entry.path().to_string_lossy();
            if is_file && file_path.ends_with(".xml") {
                if file_path.contains("releasenotes") {
                    self.releasenotes_path = file_path.to_string();
                } else {
                    self.file_paths.push(file_path.to_string());
                }
            }
        }
        Ok(())
    }

    fn find_file_idx(&self, part: &str) -> Option<usize> {
        let mut buf = Vec::new();
        for path in self.file_paths.iter().enumerate() {
            let mut reader = Reader::from_file(path.1).unwrap();
            if ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "book", end: "title", attr: Some(("xml:id", part))}).is_ok() {
                return Some(path.0); 
            }
        }
        None
    }

    fn parse_version(&mut self) -> Result<(), String> {
        let mut buf = Vec::new();
        let mut reader = Reader::from_file(&self.releasenotes_path).unwrap();
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "title", end: "title", attr: None })?;
        buf.clear();
        reader.read_event_into(&mut buf).unwrap();
        self.version = String::from_utf8_lossy(&buf).rsplit_once(' ').unwrap().1.to_string();
        Ok(())
    }

    fn parse_files(&mut self)-> Result<(), String> {
        self.parse_part_6()?;
        self.parse_part_7()?;
        Ok(())
    }

    fn parse_part_6(&mut self) -> Result<(), String> {
        let idx = self.find_file_idx("PS3.6").unwrap();
        let mut buf = Vec::new();
        let mut reader = Reader::from_file(&self.file_paths[idx]).unwrap();

        // Registry of DICOM Data Elements
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "table", end: "", attr: Some(("xml:id", "table_6-1"))})?;
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tbody", end: "td", attr: None})?;
        while ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tr", end: "tbody", attr: None}).is_ok() {
            if let Some(item) = ParseProcessor::read_row(&mut reader, &mut buf, "") {
                self.data_items.push(item);
            }
        }

        // Registry of DICOM File Meta Elements
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "table", end: "", attr: Some(("xml:id", "table_7-1"))})?;
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tbody", end: "td", attr: None})?;
        while ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tr", end: "tbody", attr: None}).is_ok() {
            if let Some(item) = ParseProcessor::read_row(&mut reader, &mut buf, "") {
                self.data_items.push(item);
            }
        }

        // Registry of DICOM Directory Structuring Elements
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "table", end: "", attr: Some(("xml:id", "table_8-1"))})?;
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tbody", end: "td", attr: None})?;
        while ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tr", end: "tbody", attr: None}).is_ok() {
            if let Some(item) = ParseProcessor::read_row(&mut reader, &mut buf, "") {
                self.data_items.push(item);
            }
        }

        // Registry of DICOM Dynamic RTP Payload Elements
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "table", end: "", attr: Some(("xml:id", "table_9-1"))})?;
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tbody", end: "td", attr: None})?;
        while ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tr", end: "tbody", attr: None}).is_ok() {
            if let Some(item) = ParseProcessor::read_row(&mut reader, &mut buf, "") {
                self.data_items.push(item);
            }
        }
        Ok(())
    }

    fn parse_part_7(&mut self) -> Result<(), String>{
        let idx = self.find_file_idx("PS3.7").unwrap();
        let mut buf = Vec::new();
        let mut reader = Reader::from_file(&self.file_paths[idx]).unwrap();

        // Registry of DICOM Command Elements
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "table", end: "", attr: Some(("xml:id", "table_E.1-1"))})?;
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tbody", end: "td", attr: None})?;
        while ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tr", end: "tbody", attr: None}).is_ok() {
            if let Some(item) = ParseProcessor::read_row(&mut reader, &mut buf, "") {
                self.data_items.push(item);
            }
        }

        // Retired Command Fields
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "table", end: "", attr: Some(("xml:id", "table_E.2-1"))})?;
        ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tbody", end: "td", attr: None})?;
        while ParseProcessor::find_element(&mut reader, &mut buf, FindElement { start: "tr", end: "tbody", attr: None}).is_ok() {
            if let Some(item) = ParseProcessor::read_row(&mut reader, &mut buf, "Ret") {
                self.data_items.push(item);
            }
        }
        Ok(())
    }

    fn find_element(reader: &mut Reader<BufReader<File>>, buf: &mut Vec<u8>, find_element: FindElement) -> Result<(), String> {
        buf.clear();
        loop {
            match reader.read_event_into(buf) {
                Ok(Event::Start(e)) => {
                    if e.name().as_ref() == find_element.start.as_bytes() {
                        if find_element.attr.is_none() {
                            return Ok(())
                        } else {
                            let (key, value) = find_element.attr.unwrap();
                            for attr in e.attributes() {
                                let attr = attr.unwrap();
                                if attr.key.as_ref() == key.as_bytes() && attr.value.as_ref() == value.as_bytes() {
                                    return Ok(());    
                                }
                            }
                        }
                    }  
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == find_element.end.as_bytes() {
                        break;
                    }  
                }
                Ok(Event::Eof) => break, 
                _ => (),
            }
            buf.clear();
        }
        
        if find_element.attr.is_some() { 
            Err(format!(r###"Xml element <{} ... {} = "{}"> not found"###, find_element.start, find_element.attr.unwrap().0, find_element.attr.unwrap().1)) 
        } else {
            Err(format!("Xml element <{}> not found", find_element.start))
        }        
    }

    fn read_row(reader: &mut Reader<BufReader<File>>, buf: &mut Vec<u8>, version: &str) -> Option<DataItem> {
        buf.clear();
        let mut count = 0;
        let mut tag = String::new();
        let mut name = String::new();
        let mut keyword = String::new();
        let mut vr = String::new();
        let mut vm = String::new();
        let mut version = if version.is_empty() {String::from("DICOM")} else { version.to_string() };
    
        while ParseProcessor::find_element(reader, buf, FindElement { start: "td", end: "tr", attr: None}).is_ok() {
            let value = ParseProcessor::read_column(reader, buf);
            match count {
                0 => tag = value,
                1 => name = value,
                2 => keyword = value,
                3 => vr = value,
                4 => vm = value,
                5 => { version = 
                    if value.starts_with("RET") {
                        "Ret".to_string()
                    } else if value == "DICOS" || value == "DICONDE" {
                        value
                    } else {
                        version
                    };
                },
                _ => (), 
            }
            count += 1;
        }
        if name.is_empty() || keyword.is_empty() || vr.is_empty() || vm.is_empty() { None } else { Some(DataItem { tag, name, keyword, vr, vm, version }) }
    }

    fn read_column(reader: &mut Reader<BufReader<File>>, buf: &mut Vec<u8>) -> String {
        buf.clear();
        let mut value = Vec::new();
        loop {
            match reader.read_event_into(buf) {
                Ok(Event::Text(e)) => {
                    value.extend_from_slice(e.as_ref());
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"td" {
                        break
                    }  
                }
                Ok(Event::Eof) => break,
                _ => (),
            }
            buf.clear();
        }
        String::from_utf8_lossy(&value).trim().to_string()
    }

    fn write_dictionary_to_file(&mut self) -> Result<(), std::io::Error> {
        self.data_items.sort_by(|a, b| a.tag.cmp(&b.tag));
        self.dictionary_header = self.dictionary_header
                .replacen("${DICOM_VERSION}", &self.version, 2)
                .replacen("${DATE}", &chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(), 1)
                .replacen("${USER}", &whoami::username(), 1)
                .replacen("${HOST}", &whoami::hostname(), 1);

        let mut file = OpenOptions::new().write(true).create(true).open(&self.config.output_file_path)?;
        file.write(self.dictionary_header.as_bytes())?;
        file.write(b"\n")?;
        for item in self.data_items.iter().enumerate() {
            file.write(format!("{}\t\"{}\"\t{}\t{}\t{}\t{}", item.1.tag, item.1.name, item.1.keyword, item.1.vr, item.1.vm, item.1.version).as_bytes())?;
            if item.0 != self.data_items.len() - 1 {
                file.write(b"\n")?;
            }
        } 
        file.flush()?;
        Ok(())
    }
}