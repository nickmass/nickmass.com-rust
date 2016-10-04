use std::error::Error;

pub fn log<T: Error>(err: T) -> ! {
    error!("{}", err);
    panic!(format!("{}", err));
}
