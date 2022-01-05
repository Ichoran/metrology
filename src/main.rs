// This file is distributed under the BSD 3-clause license.  See file LICENSE.
// Copyright (c) 2022 Rex Kerr and Calico Life Sciences LLC


use core::convert::{TryFrom, TryInto};
use std::collections::BTreeMap;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

use structopt::StructOpt;

use metrology::*;


#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "metrology", about = "Metrology computes health metrics for individual worms.")]
struct Opt {
    #[structopt(short="v", long="verbose")]
    verbose: bool,

    #[structopt(name="source", parse(from_os_str))]
    source: PathBuf,

    #[structopt(name="target", parse(from_os_str))]
    target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Dat {
    prefix: String,
    id: u32,
    path: PathBuf,
}

impl TryFrom<PathBuf> for Dat {
    type Error = io::Error;
    fn try_from(value: PathBuf) -> Result<Self, io::Error> {
        fn e<T: Into<String>>(msg: T) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, msg.into()) }

        let stem = value.file_stem().ok_or_else(|| e("filename empty"))?;
        let prefix = Path::new(stem).file_stem().and_then(|x| x.to_str()).ok_or_else(|| e("filename has no prefix"))?;
        let number = Path::new(stem).extension().and_then(|x| x.to_str()).ok_or_else(|| e("filename has no worm number"))?;
        let _suffix = value.extension().filter(|p| *p == "dat").ok_or_else(|| e("filename has no .dat extension"))?;

        let n: u32 = number.parse().map_err(|_| e("worm number isn't a number"))?;

        Ok(Dat{ prefix: prefix.into(), id: n, path: value.clone() })
    }
}

fn get_dats(path: PathBuf) -> std::io::Result<Vec<Dat>> {
    let mut files = Vec::new();
    for file in std::fs::read_dir(path)? {
        let path = file?.path();
        if !path.is_dir() {
            if let Some(p) = path.extension() {
                if p == "dat" { files.push(path.try_into()?); }
            }
        }
    }
    Ok(files)
}


fn main() {
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    let opt = Opt::from_args();
    println!("Metrology version {}", VERSION);

    let mut atomic_name = match opt.target.file_name() {
        Some(f) => f.to_string_lossy().to_string(),
        None    => { println!("Empty or invalid target directory {:?}", opt.target); std::process::exit(1) }
    };
    atomic_name.push_str(".atomic");
    let atomic_target = opt.target.with_file_name(&atomic_name);

    if   !opt.source.exists() { println!("Source directory {:?} does not exist", opt.source ); std::process::exit(1); }
    if    opt.target.exists() { println!("Target directory {:?} exists already", opt.target ); std::process::exit(1); }
    if atomic_target.exists() { println!("Temp directory {:?} exists already", atomic_target); std::process::exit(1); }

    match std::fs::create_dir_all(atomic_target.clone()) {
        Err(e) => { println!("Error creating {:?}\n{:?}", atomic_target, e); std::process::exit(1); },
        _ => ()
    }

    let mut dats = get_dats(opt.source.clone()).expect("Can't read directory");
    dats.sort();

    let mut counts: BTreeMap<String, u32> = BTreeMap::new();

    let mut dati = dats.iter();
    while let Some(d) = dati.next() {
        match counts.get_mut(&d.prefix) {
            None    => { counts.insert(d.prefix.clone(), 1u32); },
            Some(n) => *n = *n + 1,
        }
    }

    let mut key: String = String::new();
    let mut n = 0u32;

    let mut counti = counts.iter();
    while let Some((k, v)) = counti.next() {
        if *v > n {
            key = k.clone();
            n = *v;
        }
    }

    let mut rows: Vec<Scores> = Vec::new();

    for d in dats {
        if opt.verbose { println!("Found {:?}", d); }
        if key == d.prefix {
            let mut f = std::fs::File::open(d.path.clone()).unwrap();
            let mut v: Vec<u8> = Vec::new();
            match f.read_to_end(&mut v) {
                Err(e) => { println!("Error reading {:?}\n  {:?}", d.path, e); std::process::exit(1); }
                _      => ()
            }
            let data = match get_data_lines(v.as_slice()) {
                Ok(y)  => y.1,
                Err(e) => { println!("Error parsing {:?}\n  {:?}", d.path, e); std::process::exit(1) },
            };
            let area = the_area(&data);
            let midline = the_midline(&data);
            let speed1 = the_speed_in(0.0, 4.0, &data);
            let speed2 = the_speed_in(1.5, 3.5, &data);
            let xs = the_coord(|d| d.x, &data);
            let ys = the_coord(|d| d.y, &data);
            if opt.verbose {
                println!("  a  {}+-{} (n={})", area.mean(), area.error(), area.len());
                println!("  m  {}+-{} (n={})", midline.mean(), midline.error(), midline.len());
                println!("  s  {:?}", speed1);
                println!("  s' {:?}", speed2);
                println!("  x  {} -> {};  [{}, {}];  {:?}", xs.first, xs.last, xs.bound0, xs.bound1, xs.stats);
                println!("  y  {} -> {};  [{}, {}];  {:?}", ys.first, ys.last, ys.bound0, ys.bound1, ys.stats);
                println!();
            }

            rows.push(the_everything(d.id, &data));
        }
    }

    println!("Analyzed {} files from {:?}", rows.len(), opt.source);

    let mut jsonname = key.clone();
    jsonname.push_str(".scores");
    let scores_file = atomic_target.join(Path::new(&jsonname));
    match std::fs::write(scores_file.clone(), serde_json::to_string(&rows).unwrap().as_str()) {
        Err(e) => { println!("Error writing {:?}\n  {:?}", jsonname, e); std::process::exit(1); },
        _      => { println!("  Wrote {:?}", scores_file); }
    }

    if rows.len() > 0 {
        let mut csvname = key.clone();
        csvname.push_str(".csv");
        let csv_file = atomic_target.join(Path::new(&csvname));
        let mut csv = String::new();
        let mut first = true;
        for score in rows { 
            if first {
                csv.push_str(score.title().as_str());
                csv.push('\n');
                first = false;
            }
            csv.push_str(score.to_string().as_str());
            csv.push('\n');
        }
        match std::fs::write(csv_file.clone(), csv.as_str()) {
            Err(e) => { println!("Error writing {:?}\n  {:?}", csvname, e); std::process::exit(1); },
            _      => { println!("  Wrote {:?}", csv_file); }
        }
    }

    match std::fs::rename(atomic_target.clone(), opt.target.clone()) {
        Err(e) => { 
            println!("Could not move temp {:?}", atomic_target);
            println!("                 to {:?}", opt.target);
            println!("             error: {:?}", e);
            std::process::exit(1);
        }
        _      => ()
    }
}
