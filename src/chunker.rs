use std::io::{self, Read};

use cdc::{Polynom64, Rabin64, RollingHash64};

const SPLITMASK: u64 = (1u64 << 20) - 1;
const KB: usize = 1024;
const MB: usize = 1024 * KB;
const MIN_SIZE: usize = 512 * KB;
const MAX_SIZE: usize = 8 * MB;

#[inline]
fn default_predicate(x: u64) -> bool {
    (x & SPLITMASK) == 0
}

pub struct ChunkIter<R: Read> {
    buf: Vec<u8>,
    pos: usize,
    reader: R,
    predicate: fn(u64) -> bool,
    rabin: Rabin64,
    size_hint: usize,
    min_size: usize,
    max_size: usize,
    finished: bool,
}

impl<R: Read> ChunkIter<R> {
    pub fn new(reader: R, size_hint: usize, poly: &Polynom64) -> Self {
        Self {
            buf: Vec::with_capacity(4 * KB),
            pos: 0,
            reader,
            predicate: default_predicate,
            rabin: Rabin64::new_with_polynom(6, poly),
            size_hint,
            min_size: MIN_SIZE,
            max_size: MAX_SIZE,
            finished: false,
        }
    }
}

impl<R: Read> Iterator for ChunkIter<R> {
    type Item = io::Result<Vec<u8>>;

    fn next(&mut self) -> Option<io::Result<Vec<u8>>> {
        if self.finished {
            return None;
        }

        let mut min_size = self.min_size;
        let mut vec = Vec::with_capacity(self.size_hint.min(min_size));

        // check if some bytes exist in the buffer and if yes, use them
        let open_buf_len = self.buf.len() - self.pos;
        if open_buf_len > 0 {
            vec.resize(open_buf_len, 0);
            vec.copy_from_slice(&self.buf[self.pos..]);
            self.pos = self.buf.len();
            min_size -= open_buf_len;
        }

        let size = match (&mut self.reader)
            .take(min_size as u64)
            .read_to_end(&mut vec)
        {
            Ok(size) => size,
            Err(err) => return Some(Err(err)),
        };

        // If self.min_size is not reached, we are done.
        // Note that the read data is of size size + open_buf_len and self.min_size = minsize + open_buf_len
        if size < min_size {
            self.finished = true;
            vec.truncate(size + open_buf_len);
            return Some(Ok(vec));
        }

        self.rabin
            .reset_and_prefill_window(&mut vec[vec.len() - 64..vec.len()].iter().cloned());

        loop {
            if vec.len() >= self.max_size {
                break;
            }

            if self.buf.len() == self.pos {
                // TODO: use a possibly uninitialized buffer here
                self.buf.resize(4 * KB, 0);
                match self.reader.read(&mut self.buf[..]) {
                    Ok(0) => {
                        self.finished = true;
                        break;
                    }
                    Ok(size) => {
                        self.pos = 0;
                        self.buf.truncate(size);
                    }

                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        return Some(Err(e));
                    }
                }
            }

            let byte = self.buf[self.pos];
            vec.push(byte);
            self.pos += 1;
            self.rabin.slide(&byte);
            if (self.predicate)(self.rabin.hash) {
                break;
            }
        }
        self.size_hint -= vec.len();
        Some(Ok(vec))
    }
}
