extern crate fs_extra;
use fs_extra::dir::{copy, move_dir, remove, CopyOptions};

pub fn backup_src() {
    remove("./src-saved").ok();

    let mut options = CopyOptions::new();
    options.copy_inside = true;

    copy("./src", "./src-saved", &options).unwrap();
}

pub fn restore_src() {
    let mut options = CopyOptions::new();
    options.copy_inside = true;

    remove("./src-modified").ok();
    move_dir("./src", "./src-modified", &options).unwrap();
    move_dir("./src-saved", "./src", &options).unwrap();
}
