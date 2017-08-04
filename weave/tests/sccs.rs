/// Comparisons against SCCS.
///
/// The weave algorithm used comes from the the SCCS program.  This can be installed on most Linux
/// distros by installing the package "cssc".

extern crate env_logger;
#[macro_use] extern crate log;
extern crate rand;
extern crate tempdir;
extern crate weave;

use rand::{Rng, SeedableRng, StdRng};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::fs::{File, remove_file};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::rc::Rc;
use tempdir::TempDir;
use weave::{DeltaWriter, NewWeave, Parser, SimpleNaming, Sink, Result};

#[test]
fn sccs() {
    let _ = env_logger::init();

    if !has_sccs() {
        return;
    }

    let tdir = TempDir::new("sccstest").unwrap();
    let mut gen = Gen::new(tdir.path()).unwrap();

    // For debugging, this will cause the directory to not be removed.
    if env::var("KEEPTEMP").is_ok() {
        tdir.into_path();
    }

    gen.new_sccs();
    gen.new_weave();
    gen.weave_check();

    for i in 0 .. 100 {
        gen.shuffle();
        gen.add_sccs_delta();
        gen.add_weave_delta(i + 1);

        // Checking with sccs is very slow.  Do we want to do it?
        // gen.sccs_check();
        gen.weave_check();
    }
}

/// Determine if we have the sccs command available.  If not, show an error, and return false.
fn has_sccs() -> bool {
    match Command::new("sccs").arg("-V").output() {
        Ok(_) => true,
        Err(_) => {
            error!("'sccs' not found in path, skipping tests, install 'cssc' to fix");
            false
        }
    }
}

/// Gen synthesizes a series of deltas, and can add them using SCCS to make a weave file, and later
/// to this weave implementation to compare the results.
struct Gen {
    /// The directory to write the files into.
    tdir: PathBuf,

    /// The name of the plain file related to it.
    sccs_plain: PathBuf,

    /// The current lines.
    nums: Vec<usize>,

    /// Each delta.  Sccs numbers the deltas from 1, so these are off by one.
    deltas: Vec<Vec<usize>>,

    /// A Rng for generating the shuffles.
    rand: StdRng,
}

impl Gen {
    fn new<P: AsRef<Path>>(tdir: P) -> Result<Gen> {
        let tdir = tdir.as_ref();
        let seed: &[_] = &[1, 2, 3, 4];
        Ok(Gen {
            tdir: tdir.to_owned(),
            sccs_plain: tdir.join("tfile"),
            nums: (1..101).collect(),
            rand: SeedableRng::from_seed(seed),
            deltas: vec![],
        })
    }

    /// Perform a somewhat random modification of the data.  Choose some range of the numbers and
    /// reverse them.
    fn shuffle(&mut self) {
        let a = self.rand.gen_range(0, self.nums.len());
        let b = self.rand.gen_range(0, self.nums.len());

        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        self.nums[a..b].reverse();
    }

    /// Write to a new sccs file, resulting in delta 1.
    fn new_sccs(&mut self) {
        self.emit_to(&self.sccs_plain);
        Command::new("sccs").args(&["admin", "-itfile", "-n", "s.tfile"])
            .current_dir(&self.tdir)
            .status()
            .expect("Unable to run sccs admin")
            .expect_success("Sccs command returned error");
        remove_file(&self.sccs_plain).expect("Unable to remove data file");

        /// Consider the deltas as canonical by the SCCS command, so store the first delta.
        self.deltas.push(self.nums.clone());
    }

    /// Add a new delta to the sccs file.
    fn add_sccs_delta(&mut self) {
        Command::new("sccs").args(&["get", "-e", "s.tfile"])
            .current_dir(&self.tdir)
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .expect("Unable to run sccs get")
            .expect_success("sccs get failed");
        self.emit_to(&self.sccs_plain);
        Command::new("sccs").args(&["delta", "-yMessage", "s.tfile"])
            .current_dir(&self.tdir)
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .expect("Unable to run sccs delta")
            .expect_success("sccs delta failed");

        self.deltas.push(self.nums.clone());
    }

    /// Emit the current numbers to the given name (in the temp dir).
    fn emit_to<P: AsRef<Path>>(&self, name: P) {
        let mut fd = File::create(self.tdir.join(name)).unwrap();
        for i in &self.nums {
            writeln!(&mut fd, "{}", i).unwrap();
        }
    }

    /// Check the output of "sccs get".  This is more of a sanity check.
    #[allow(dead_code)]
    fn sccs_check(&self) {
        for (i, del) in self.deltas.iter().enumerate() {
            self.sccs_check_one(i, del);
        }
    }

    #[allow(dead_code)]
    fn sccs_check_one(&self, num: usize, data: &[usize]) {
        let out = Command::new("sccs").args(&["get", &format!("-r1.{}", num+1), "-p", "s.tfile"])
            .current_dir(&self.tdir)
            .output()
            .expect("Unable to run sccs get");
        out.status.expect_success("Error running sccs get");
        let mut onums: Vec<usize> = vec![];
        for line in BufReader::new(&out.stdout[..]).lines() {
            let line = line.unwrap();
            onums.push(line.as_str().parse::<usize>().unwrap());
        }

        assert_eq!(data, &onums[..]);
    }

    /// Check that weave decodes all of the sccs files properly.
    fn weave_check(&self) {
        for (i, del) in self.deltas.iter().enumerate() {
            self.weave_sccs_check_one(i, del);
            self.weave_check_one(i, del);
        }
    }

    fn weave_sccs_check_one(&self, num: usize, data: &[usize]) {
        let fd = File::open(self.tdir.join("s.tfile")).unwrap();
        let lines = BufReader::new(fd).lines();
        let dsink = Rc::new(RefCell::new(DeltaSink { nums: vec![] }));
        {
            let mut parser = Parser::new(lines, dsink.clone(), num + 1);
            match parser.parse_to(0) {
                Ok(0) => (),
                Ok(_) => panic!("Unexpected stop of parser"),
                Err(e) => panic!("Parser error: {:?}", e),
            }
        }

        assert_eq!(data, &dsink.borrow().nums[..]);
    }

    fn weave_check_one(&self, num: usize, data: &[usize]) {
        let fd = File::open(self.tdir.join("sample.weave")).unwrap();
        let lines = BufReader::new(fd).lines();
        let dsink = Rc::new(RefCell::new(DeltaSink { nums: vec![] }));
        {
            let mut parser = Parser::new(lines, dsink.clone(), num + 1);
            match parser.parse_to(0) {
                Ok(0) => (),
                Ok(_) => panic!("Unexpected stop of parser"),
                Err(e) => panic!("Parser error: {:?}", e),
            }
        }

        assert_eq!(data, &dsink.borrow().nums[..]);
    }

    fn new_weave(&mut self) {
        let mut tags = BTreeMap::new();
        tags.insert("name", "initial");
        let nc = SimpleNaming::new(&self.tdir, "sample", "weave", false);
        let mut nw = NewWeave::new(&nc, tags.into_iter()).unwrap();
        for i in &self.nums {
            writeln!(&mut nw, "{}", i).unwrap();
        }
        nw.close().unwrap();
    }

    fn add_weave_delta(&mut self, base: usize) {
        let name_value = format!("{}", base + 1);
        let mut tags = BTreeMap::new();
        tags.insert("name", name_value.as_str());
        let nc = SimpleNaming::new(&self.tdir, "sample", "weave", false);
        let mut delta = DeltaWriter::new(&nc, tags.into_iter(), base).unwrap();
        for i in &self.nums {
            writeln!(&mut delta, "{}", i).unwrap();
        }
        delta.close().unwrap();
    }
}

/// A Weave Sink that just collects the numbers in the given delta.
struct DeltaSink {
    nums: Vec<usize>,
}

impl Sink for DeltaSink {
    fn plain(&mut self, text: &str, keep: bool) -> Result<()> {
        if !keep {
            return Ok(())
        }

        self.nums.push(text.parse::<usize>()?);
        Ok(())
    }
}

/// A small utility to make asserting success easier.
trait Successful {
    fn expect_success(&self, msg: &str);
}

impl Successful for ExitStatus {
    fn expect_success(&self, msg: &str) {
        if !self.success() {
            panic!(msg.to_string());
        }
    }
}