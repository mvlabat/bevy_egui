fn main() {
    println!("cargo::rustc-check-cfg=cfg(web_sys_unstable_apis)");
}
