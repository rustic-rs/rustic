use std::io::{self, Read};
use std::slice;

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
    reader: R,
    predicate: fn(u64) -> bool,
    rabin: Rabin64,
    min_size: usize,
    max_size: usize,
    finished: bool,
}

impl<R: Read> ChunkIter<R> {
    pub fn new(reader: R, poly: &Polynom64) -> Self {
        Self {
            reader,
            predicate: default_predicate,
            rabin: Rabin64::new_with_polynom(6, poly),
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

        let mut vec = Vec::new();
        let size = match (&mut self.reader)
            .take(self.min_size.try_into().unwrap())
            .read_to_end(&mut vec)
        {
            Ok(size) => size,
            Err(err) => return Some(Err(err)),
        };

        if size < self.min_size {
            self.finished = true;
            return Some(Ok(vec));
        }

        let mut index = self.min_size;
        self.rabin
            .reset_and_prefill_window(&mut vec[self.min_size - 64..self.min_size].iter().cloned());

        let mut byte = 0;

        loop {
            if index >= self.max_size {
                return Some(Ok(vec));
            }
            match self.reader.read(slice::from_mut(&mut byte)) {
                Ok(0) => {
                    self.finished = true;
                    return Some(Ok(vec));
                }
                Ok(..) => {
                    vec.push(byte);
                    index += 1;
                    self.rabin.slide(&byte);
                    if (self.predicate)(self.rabin.hash) {
                        return Some(Ok(vec));
                    }
                }

                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Some(Err(e));
                }
            }
        }
    }
}
