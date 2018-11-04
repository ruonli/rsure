// Playing with paths.

#![warn(bare_trait_objects)]

extern crate chrono;
extern crate env_logger;
extern crate regex;
extern crate rsure;

#[macro_use]
extern crate failure;

#[macro_use]
extern crate log;

extern crate structopt;

use chrono::Local;
use std::{collections::BTreeMap, path::Path};
use structopt::StructOpt;

use rsure::{
    parse_store, show_tree, stdout_visitor, Progress, StoreTags, StoreVersion, SureHash,
    TreeCompare, Version,
};

mod bkcmd;

// For now, just use the crate's error type.
pub use rsure::Result;

#[derive(StructOpt)]
#[structopt(name = "rsure", about = "File integrity")]
struct Opt {
    #[structopt(short = "f", long = "file", default_value = "2sure.dat.gz")]
    /// Base of file name, default 2sure, will get .dat.gz appended
    file: String,
    #[structopt(short = "d", long = "dir", default_value = ".")]
    /// Directory to scan, defaults to "."
    dir: String,
    #[structopt(long = "tag")]
    /// key=value to associate with scan
    tag: Vec<String>,
    #[structopt(short = "v", long = "version")]
    version: Option<String>,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "scan")]
    /// Scan a directory for the first time
    Scan,
    #[structopt(name = "update")]
    /// Update the scan using the dat/weave file
    Update,
    #[structopt(name = "check")]
    /// Compare the directory with the dat/weave file
    Check,
    #[structopt(name = "signoff")]
    /// Compare dat with bak file, or last two versions in weave file
    Signoff,
    #[structopt(name = "show")]
    /// Pretty print the dat file
    Show,
    #[structopt(name = "bknew")]
    /// Create a new bitkeeper-based sure store
    BkNew { dir: String },
    #[structopt(name = "bkimport")]
    /// Import a tree of surefiles into a bk store
    BkImport {
        #[structopt(long = "src")]
        src: String,
        #[structopt(long = "dest")]
        dest: String,
    },
    #[structopt(name = "list")]
    /// List revisions in a given sure store
    List,
}

#[allow(dead_code)]
fn main() {
    env_logger::init();

    let opt = Opt::from_args();

    let store = parse_store(&opt.file).unwrap();

    let mut tags = decode_tags(Some(opt.tag.iter().map(|x| x.as_str())));

    add_name_tag(&mut tags, &opt.dir);

    // Note that only the "check" command uses the version tag.
    let latest = match opt.version {
        None => Version::Latest,
        Some(x) => Version::Tagged(x.to_string()),
    };

    match opt.command {
        Command::Scan => {
            rsure::update(&opt.dir, &*store, false, &tags).unwrap();
        }
        Command::Update => {
            rsure::update(&opt.dir, &*store, true, &tags).unwrap();
        }
        Command::Check => {
            let old_tree = store.load(latest).unwrap();
            let mut new_tree = rsure::scan_fs(&opt.dir).unwrap();
            let estimate = new_tree.hash_estimate();
            let pdir = &Path::new(&opt.dir);
            let mut progress = Progress::new(estimate.files, estimate.bytes);
            new_tree.hash_update(pdir, &mut progress);
            progress.flush();
            info!("check {:?}", opt.file);
            new_tree.compare_from(&mut stdout_visitor(), &old_tree, pdir);
        }
        Command::Signoff => {
            let old_tree = store.load(Version::Prior).unwrap();
            let new_tree = store.load(Version::Latest).unwrap();
            println!("signoff {}", opt.file);
            new_tree.compare_from(&mut stdout_visitor(), &old_tree, &Path::new(&opt.dir));
        }
        Command::Show => {
            println!("show {}", opt.file);
            show_tree(&Path::new(&opt.file)).unwrap();
        }
        Command::BkNew { ref dir } => {
            bkcmd::new(dir).unwrap();
        }
        Command::BkImport { ref src, ref dest } => {
            bkcmd::import(src, dest).unwrap();
        }
        Command::List => {
            let version = store.get_versions().unwrap();
            dump_versions(&version);
        }
    }
}

/// Decode the command-line tags.  Tags should be of the form key=value, and multiple can be
/// specified, terminated by the command.  It is also possible to specify --tag multiple times.
fn decode_tags<'a, I>(tags: Option<I>) -> StoreTags
where
    I: Iterator<Item = &'a str>,
{
    match tags {
        None => BTreeMap::new(),
        Some(tags) => tags.map(|x| decode_tag(x)).collect(),
    }
}

fn decode_tag<'a>(tag: &'a str) -> (String, String) {
    let fields: Vec<_> = tag.splitn(2, '=').collect();
    if fields.len() != 2 {
        panic!("Tag must be key=value");
    }
    (fields[0].to_string(), fields[1].to_string())
}

/// If the caller doesn't specify a 'name=' tag, generate one based on the current timestamp.
/// Also will add a 'dir' attribute for where the tree was captured.
fn add_name_tag<P: AsRef<Path>>(tags: &mut StoreTags, dir: P) {
    if !tags.contains_key("name") {
        tags.insert("name".to_string(), Local::now().to_rfc3339());
    }

    if !tags.contains_key("dir") {
        tags.insert(
            "dir".to_string(),
            dir.as_ref()
                .canonicalize()
                .unwrap_or_else(|_| Path::new("invalid").to_owned())
                .to_string_lossy()
                .into_owned(),
        );
    }
}

fn dump_versions(versions: &[StoreVersion]) {
    println!("vers | Time captured       | name");
    println!("-----+---------------------+------------------");
    for v in versions {
        let vers = match v.version {
            Version::Latest => "tip",
            Version::Prior => "prev",
            Version::Tagged(ref v) => v,
        };
        println!(
            "{:4} | {} | {}",
            vers,
            v.time.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S"),
            v.name
        );
    }
}
