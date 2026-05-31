fn main() {
    pkg_config::Config::new()
        .atleast_version("1.50")
        .probe("espeak-ng")
        .expect("espeak-ng not found. Install via: brew install espeak-ng");
}
