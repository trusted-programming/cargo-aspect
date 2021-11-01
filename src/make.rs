use crate::config::{Config, PointCut};
use adjacent_pair_iterator::AdjacentPairIterator;
use regex::Regex;
use std::collections::{BinaryHeap, HashMap};
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::iter::once;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::cmp::Ordering;

const ASPECT_OUTPUT_FILE: &'static str = "RUST_ASPECT_OUTPUT.txt";

pub fn build_proj(c: &Config) {
    // modify source file
    for pc in &c.pointcuts {
        let inspect_str = format!(r#"aop-inspect="{}""#, pc.condition);
        let _ = Command::new("cargo")
            .arg("+AOP")
            .arg("rustc")
            .arg("--")
            .arg("-Z")
            .arg(&inspect_str)
            .status()
            .expect("failed to execute rustc process");
        let out_files = find_aop_output_file();
        for out_file in &out_files {
            let content = std::fs::read(out_file).expect("read file failed.");
            let content = String::from_utf8_lossy(&content);
            let parsed_output = parse_aop_outputs(&content);
            for (file, found) in &parsed_output {
                let origin = read_file(file);
                let updated = insert_advice(origin, found, &pc);
                write_file(file, updated);
            }
            std::fs::remove_file(out_file).ok();
        }
    }
    // build the modified source
}

fn find_aop_output_file() -> Vec<PathBuf> {
    let mut root = super::config::get_root().expect("failed to found root folder");
    root.push("target");
    let mut res = Vec::new();
    visit_dirs(&root, &mut res).ok();
    return res;
}

fn visit_dirs(dir: &Path, res: &mut Vec<PathBuf>) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, res)?;
            } else if path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(ASPECT_OUTPUT_FILE)
            {
                res.push(path.into());
            }
        }
    }
    Ok(())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Pos {
    line: usize,
    col: usize,
}

#[derive(Clone, Debug)]
struct Found {
    file: String,
    src: String,
    start: Pos,
    end: Pos,
    args: HashMap<String, String>,
}

impl Ord for Found {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl PartialOrd for Found {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.start.partial_cmp(&other.start)
    }
}

impl PartialEq for Found {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start
    }
}

impl Eq for Found {}

fn parse_aop_outputs(s: &str) -> HashMap<String, BinaryHeap<Found>> {
    let mut res = HashMap::<String, BinaryHeap<Found>>::new();

    let founds: Vec<usize> = s
        .match_indices("Found {")
        .map(|x| x.0)
        .chain(once(s.len()))
        .collect();

    for (&from, &to) in founds.iter().adjacent_pairs() {
        let sub = &s[from..to];
        let f = parse_found(sub);
        res.entry(f.file.clone()).or_insert(BinaryHeap::new()).push(f);
    }
    return res;
}

fn parse_found(s: &str) -> Found {
    let re = Regex::new(r#"([^\s]+\.rs):(\d+):(\d+):\s+(\d+):(\d+)"#).expect("regex error");
    let m = re.captures_iter(s).next().expect("Parse Found error!");
    let file = m.get(1).unwrap().as_str().to_string();

    let line1 = m.get(2).unwrap().as_str().parse::<usize>().unwrap();
    let col1 = m.get(3).unwrap().as_str().parse::<usize>().unwrap();
    let start = Pos {
        line: line1,
        col: col1,
    };
    let line2 = m.get(4).unwrap().as_str().parse::<usize>().unwrap();
    let col2 = m.get(5).unwrap().as_str().parse::<usize>().unwrap();
    let end = Pos {
        line: line2,
        col: col2,
    };

    let mut src = String::new();
    for l in s.lines() {
        if l.contains("src:") {
            if let Some(split) = l.find(':') {
                let value = l[(split+1)..].trim_matches(|c| c ==' ' || c == '"' || c == ',');
                src = value.replace('\\', "");
            }
            break;
        }
    }

    let mut args = HashMap::new();
    let mut arg_line = false;
    for l in s.lines() {
        if l.contains("args:") {
            arg_line = true;
            continue;
        }
        if !arg_line {
            continue;
        }
        if let Some(split) = l.find(':') {
            let key = l[0..split].trim_matches(|c| c == ' '|| c == '"');
            let value = l[(split+1)..].trim_matches(|c| c ==' ' || c == '"' || c == ',');
            let value = value.replace('\\', "");
            args.insert(key.to_string(), value);
        }
    }
    let f = Found {
        file,
        src,
        start,
        end,
        args
    };

    return f;
}

fn read_file(f: &str) -> String {
    let f = File::open(f).expect(&format!("file open failed: {}", f));
    let mut reader = BufReader::new(f);
    let mut buffer = String::new();
    reader.read_to_string(&mut buffer).ok();
    return buffer;
}

fn write_file(f: &str, content: String) {
    let file = File::create(f).expect("write file failed");
    let mut file = BufWriter::new(file);
    file.write_all(content.as_bytes()).unwrap();
    file.flush().ok();
}

fn insert_advice(mut src: String, founds: &BinaryHeap<Found>, pc: &PointCut) -> String {

    let mut founds = founds.clone();
    while let Some(f) = founds.pop() {
        let from = find_index_by_pos(&src, f.start);
        let to = find_index_by_pos(&src, f.end);

        let mut advice = pc.advice.clone();
        for (k, v) in &f.args {
            advice = advice.replace(k, v);
        }
        advice = advice.replace('$', &f.src);

        let mut new_str = String::new();
        new_str.push_str(&src[0..from]);
        new_str.push_str(&advice);
        new_str.push_str(&src[to..]);
        src = new_str;
    }

    return src;
}

fn find_index_by_pos(src: &str, pos: Pos) -> usize {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in src.bytes().enumerate() {
        if line == pos.line && col == pos.col {
            return i;
        }

        if c == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    panic!("Line {} and Column {} is not found", pos.line, pos.col);
}
