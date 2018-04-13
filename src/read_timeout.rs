use memchr;
use std::io::{BufRead, Error, ErrorKind, Result};

// -------------------------------------------------------------------------------------------------
// Functions
// -------------------------------------------------------------------------------------------------

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

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

pub fn read_line_timeout<R: BufRead + ?Sized>(r: &mut R, buf: &mut String) -> Result<(usize, bool)> {
    append_to_string(buf, |b| read_until_timeout(r, b'\n', b))
}

fn append_to_string<F>(buf: &mut String, f: F) -> Result<(usize, bool)>
where
    F: FnOnce(&mut Vec<u8>) -> Result<(usize, bool)>,
{
    unsafe {
        let mut g = Guard {
            len: buf.len(),
            buf: buf.as_mut_vec(),
        };
        let ret = f(g.buf);
        if String::from_utf8(g.buf[g.len..].to_vec()).is_err() {
            ret.and_then(|_| {
                Err(Error::new(
                    ErrorKind::InvalidData,
                    "stream did not contain valid UTF-8",
                ))
            })
        } else {
            g.len = g.buf.len();
            ret
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------
