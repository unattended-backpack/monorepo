use std::env;

fn main() {
    println!("cargo:rerun-if-changed=.cargo/config.toml");
    match env::var("BUILD_SCRIPT_USED") {
        Ok(used) if used == "1" => {}
        _ => {
            panic!("Please build using the provided `build.sh` script!");
        }
    }
}
