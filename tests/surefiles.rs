// Test the rsure API for save and load.

extern crate rsure;
extern crate tempdir;

use rsure::{SureTree, TreeCompare};
use tempdir::TempDir;

// Test that the API is usable.  Currently, the output only generates a
// report to stdout, and doesn't return any information to the caller, so
// we can only test that the calls work.  If you run the test with
// "--nocapture", it should show the addition of the surefile at the end.
#[test]
fn save_and_load() {
    let tmp = TempDir::new("rsure").unwrap();
    let tree = rsure::scan_fs(tmp.path()).unwrap();

    // First surefile.
    let sfile = tmp.path().join("surefile.dat.gz");

    // Save it to a file.
    tree.save(&sfile).unwrap();

    // Load it back in.
    let t2 = SureTree::load(&sfile).unwrap();
    t2.compare_from(&tree, &sfile);

    // Rescan (should catch the newly added surefile).
    let t3 = rsure::scan_fs(tmp.path()).unwrap();
    t3.compare_from(&t2, tmp.path());
}