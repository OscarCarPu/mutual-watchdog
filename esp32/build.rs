use dotenvy::from_filename_iter;

fn main() {
    embuild::espidf::sysenv::output();

    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=../.env");

    for item in from_filename_iter(".env").expect("failed to read .env file") {
        let (key, value) = item.expect("failed to parse .env entry");
        println!("cargo:rustc-env={key}={value}");
    }

    for item in from_filename_iter("../.env").expect("failed to read root .env file") {
        let (key, value) = item.expect("failed to parse root .env entry");
        println!("cargo:rustc-env={key}={value}");
    }
}
