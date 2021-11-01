
mod config;
mod make;
mod src_mgr;

fn main() {
    println!("=== Cargo Aspect ===");
    let c = config::parse_config();
    src_mgr::backup_src();
    make::build_proj(&c);
    src_mgr::restore_src();
}
