use memchr;
use std::io::{BufRead, ErrorKind, Result};

// -------------------------------------------------------------------------------------------------
// Functions
// -------------------------------------------------------------------------------------------------

pub fn read_until_timeout<R: BufRead + ?Sized>(
    r: &mut R,
    delim: u8,
    buf: &mut Vec<u8>,
) -> Result<(usize, bool)> {
    let mut read = 0;
    let empty = vec![];
    loop {
        let (done, used, timeout) = {
            let (available, timeout) = match r.fill_buf() {
                Ok(n) => (n, false),
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(ref e) if e.kind() == ErrorKind::TimedOut => (&empty as &[u8], true),
                Err(e) => return Err(e),
            };
            match memchr::memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..i + 1]);
                    (true, i + 1, timeout)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len(), timeout)
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok((read, timeout));
        }
    }
}

pub fn read_line_timeout<R: BufRead + ?Sized>(
    r: &mut R,
    buf: &mut Vec<u8>,
) -> Result<(usize, bool)> {
    read_until_timeout(r, b'\n', buf)
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------
