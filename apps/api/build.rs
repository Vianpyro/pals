// migrate!() tracks the migration it read, not the directory: a new one needs this rebuild
fn main() {
    println!("cargo::rerun-if-changed=../../db/migrations");
}
