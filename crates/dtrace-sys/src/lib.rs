#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    // Disabled by default as the test fails without sudo.
    #[ignore]
    #[test]
    fn dtrace_open_close() {
        let flags = 0i32;
        let mut error = 0i32;
        unsafe {
            let dtrace = dtrace_open(DTRACE_VERSION as i32, flags, &mut error);
            assert_eq!(error, 0); // this will fail without sudo
            dtrace_close(dtrace);
        }
    }
}
