use dotenvy::dotenv_iter;

fn main() {
    embuild::espidf::sysenv::output();

    println!("cargo:rerun-if-changed=.env");

    for item in dotenv_iter().expect("failed to read .env file") {
        let (key, value) = item.expect("failed to parse .env entry");
        println!("cargo:rustc-env={key}={value}");
    }
}
