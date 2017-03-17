// Playing with paths.

extern crate rsure;
extern crate env_logger;
extern crate regex;

#[macro_use]
extern crate log;

#[macro_use]
extern crate clap;

use clap::{App, AppSettings, Arg, SubCommand};

use std::path::Path;

use rsure::{show_tree, Progress, SureHash, TreeCompare, stdout_visitor, parse_store, Version};

mod bkcmd;

// For now, just use the crate's error type.
pub use rsure::Result;

#[allow(dead_code)]
fn main() {
    env_logger::init().unwrap();

    let matches = App::new("rsure")
        .version(crate_version!())
        .setting(AppSettings::GlobalVersion)
        .arg(Arg::with_name("file")
             .short("f")
             .long("file")
             .takes_value(true)
             .help("Base of file name, default 2sure, will get .dat.gz appended"))
        .arg(Arg::with_name("src")
             .short("s")
             .long("src")
             .takes_value(true)
             .help("Source .dat file for signoff"))
        .arg(Arg::with_name("old")
             .short("o")
             .long("old")
             .takes_value(true)
             .help("Source .dat for update"))
        .arg(Arg::with_name("dir")
             .short("d")
             .long("dir")
             .takes_value(true)
             .help("Directory to scan, defaults to \".\""))
        .setting(AppSettings::SubcommandRequired)
        .subcommand(SubCommand::with_name("scan")
                    .about("Scan a directory for the first time"))
        .subcommand(SubCommand::with_name("update")
                    .about("Update the scan using the dat file"))
        .subcommand(SubCommand::with_name("check")
                    .about("Compare the directory with the dat file"))
        .subcommand(SubCommand::with_name("signoff")
                    .about("Compare the dat file with the bak file"))
        .subcommand(SubCommand::with_name("show")
                    .about("Pretty print the dat file"))
        .subcommand(SubCommand::with_name("bknew")
                    .about("Create a new bitkeeper-based sure store")
                    .arg(Arg::with_name("dir")
                         .required(true)
                         .help("Directory to create bk-based store")))
        .subcommand(SubCommand::with_name("bkimport")
                    .about("Import a tree of surefiles into a bk store")
                    .arg(Arg::with_name("src")
                         .long("src")
                         .takes_value(true)
                         .required(true))
                    .arg(Arg::with_name("dest")
                         .long("dest")
                         .takes_value(true)
                         .required(true)))
        .get_matches();

    let dir = matches.value_of("dir").unwrap_or(".");

    let file = matches.value_of("file").unwrap_or("2sure.dat.gz");
    let store = parse_store(file).unwrap();

    match matches.subcommand() {
        ("scan", Some(_)) => {
            rsure::update(&dir, &*store, false).unwrap();
        },
        ("update", Some(_)) => {
            rsure::update(&dir, &*store, true).unwrap();
        },
        ("check", Some(_)) => {
            let old_tree = store.load(Version::Latest).unwrap();
            let mut new_tree = rsure::scan_fs(&dir).unwrap();
            let estimate = new_tree.hash_estimate();
            let pdir = &Path::new(dir);
            let mut progress = Progress::new(estimate.files, estimate.bytes);
            new_tree.hash_update(pdir, &mut progress);
            progress.flush();
            info!("check {:?}", file);
            new_tree.compare_from(&mut stdout_visitor(), &old_tree, pdir);
        },
        ("signoff", Some(_)) => {
            let old_tree = store.load(Version::Prior).unwrap();
            let new_tree = store.load(Version::Latest).unwrap();
            println!("signoff {}", file);
            new_tree.compare_from(&mut stdout_visitor(), &old_tree, &Path::new(dir));
        },
        ("show", Some(_)) => {
            println!("show {}", file);
            show_tree(&Path::new(&file)).unwrap();
        },
        ("bknew", Some(sub)) => {
            let bkdir = sub.value_of("dir").unwrap();
            bkcmd::new(bkdir).unwrap();
        },
        ("bkimport", Some(sub)) => {
            let src = sub.value_of("src").unwrap();
            let dest = sub.value_of("dest").unwrap();
            bkcmd::import(src, dest).unwrap();
        },
        _ => {
            panic!("Unsupported command.");
        }
    }
}
