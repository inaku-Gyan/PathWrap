//! Console fixture: observe the actual Rust Windows argv parser without
//! creating a UI or registering any startup item.
fn main() {
    match pathwarp::config::parse_storage_dir(std::env::args_os().skip(1)) {
        Ok(Some(storage)) => {
            use std::io::Write;
            if std::io::stdout()
                .write_all(storage.to_string_lossy().as_bytes())
                .is_err()
            {
                std::process::exit(1);
            }
        }
        _ => std::process::exit(2),
    }
}
