use std::io::{self, Read};

use rand::{thread_rng, Rng};

use crate::{
    cdc::{
        polynom::{Polynom, Polynom64},
        rolling_hash::{Rabin64, RollingHash64},
    },
    error::PolynomialErrorKind,
    RusticResult,
};

pub(super) mod constants {
    pub(super) const SPLITMASK: u64 = (1u64 << 20) - 1;
    pub(super) const KB: usize = 1024;
    pub(super) const MB: usize = 1024 * KB;
    pub(super) const MIN_SIZE: usize = 512 * KB;
    pub(super) const MAX_SIZE: usize = 8 * MB;
    pub(super) const BUF_SIZE: usize = 64 * KB;
    pub(super) const RAND_POLY_MAX_TRIES: i32 = 1_000_000;
}

#[inline]
const fn default_predicate(x: u64) -> bool {
    (x & constants::SPLITMASK) == 0
}

pub(crate) struct ChunkIter<R: Read + Send> {
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

impl<R: Read + Send> ChunkIter<R> {
    pub(crate) fn new(reader: R, size_hint: usize, rabin: Rabin64) -> Self {
        Self {
            buf: Vec::with_capacity(4 * constants::KB),
            pos: 0,
            reader,
            predicate: default_predicate,
            rabin,
            size_hint, // size hint is used to optimize memory allocation; this should be an upper bound on the size
            min_size: constants::MIN_SIZE,
            max_size: constants::MAX_SIZE,
            finished: false,
        }
    }
}

impl<R: Read + Send> Iterator for ChunkIter<R> {
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
            return if vec.is_empty() { None } else { Some(Ok(vec)) };
        }

        _ = self
            .rabin
            .reset_and_prefill_window(&mut vec[vec.len() - 64..vec.len()].iter().copied());

        loop {
            if vec.len() >= self.max_size {
                break;
            }

            if (self.predicate)(self.rabin.hash) {
                break;
            }

            if self.buf.len() == self.pos {
                // TODO: use a possibly uninitialized buffer here
                self.buf.resize(constants::BUF_SIZE, 0);
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
            self.rabin.slide(byte);
        }
        self.size_hint -= vec.len();
        Some(Ok(vec))
    }
}

/// [`random_poly`] returns an random irreducible polynomial of degree 53
/// (largest prime number below 64-8)
/// There are (2^53-2/53) irreducible polynomials of degree 53 in
/// `F_2[X]`, c.f. Michael O. Rabin (1981): "Fingerprinting by Random
/// Polynomials", page 4. If no polynomial could be found in one
/// million tries, an error is returned.
pub fn random_poly() -> RusticResult<u64> {
    for _ in 0..constants::RAND_POLY_MAX_TRIES {
        let mut poly: u64 = thread_rng().gen();

        // mask away bits above bit 53
        poly &= (1 << 54) - 1;

        // set highest and lowest bit so that the degree is 53 and the
        // polynomial is not trivially reducible
        poly |= (1 << 53) | 1;

        if poly.irreducible() {
            return Ok(poly);
        }
    }
    Err(PolynomialErrorKind::NoSuitablePolynomialFound.into())
}

pub(crate) trait PolynomExtend {
    fn irreducible(&self) -> bool;
    fn gcd(self, other: Self) -> Self;
    fn add(self, other: Self) -> Self;
    fn mulmod(self, other: Self, modulo: Self) -> Self;
}

// implementation goes along the lines of
// https://github.com/restic/chunker/blob/master/polynomials.go
impl PolynomExtend for Polynom64 {
    // Irreducible returns true iff x is irreducible over F_2. This function
    // uses Ben Or's reducibility test.
    //
    // For details see "Tests and Constructions of Irreducible Polynomials over
    // Finite Fields".
    fn irreducible(&self) -> bool {
        for i in 1..=self.degree() / 2 {
            if self.gcd(qp(i, *self)) != 1 {
                return false;
            }
        }
        true
    }

    fn gcd(self, other: Self) -> Self {
        if other == 0 {
            return self;
        }

        if self == 0 {
            return other;
        }

        if self.degree() < other.degree() {
            self.gcd(other.modulo(self))
        } else {
            other.gcd(self.modulo(other))
        }
    }

    fn add(self, other: Self) -> Self {
        self ^ other
    }

    fn mulmod(self, other: Self, modulo: Self) -> Self {
        if self == 0 || other == 0 {
            return 0;
        }

        let mut res: Self = 0;
        let mut a = self;
        let mut b = other;

        if b & 1 > 0 {
            res = res.add(a).modulo(modulo);
        }

        while b != 0 {
            a = (a << 1).modulo(modulo);
            b >>= 1;
            if b & 1 > 0 {
                res = res.add(a).modulo(modulo);
            }
        }

        res
    }
}

// qp computes the polynomial (x^(2^p)-x) mod g. This is needed for the
// reducibility test.
fn qp(p: i32, g: Polynom64) -> Polynom64 {
    // start with x
    let mut res: Polynom64 = 2;

    for _ in 0..p {
        // repeatedly square res
        res = res.mulmod(res, g);
    }

    // add x
    res.add(2).modulo(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{repeat, Cursor};

    #[test]
    fn chunk_empty() {
        let empty: Vec<u8> = vec![];
        let mut reader = Cursor::new(empty);

        let poly = random_poly().unwrap();
        let rabin = Rabin64::new_with_polynom(6, poly);
        let chunker = ChunkIter::new(&mut reader, 0, rabin);

        assert_eq!(0, chunker.into_iter().count());
    }

    #[test]
    fn chunk_empty_wrong_hint() {
        let empty: Vec<u8> = vec![];
        let mut reader = Cursor::new(empty);

        let poly = random_poly().unwrap();
        let rabin = Rabin64::new_with_polynom(6, poly);
        let chunker = ChunkIter::new(&mut reader, 100, rabin);

        assert_eq!(0, chunker.into_iter().count());
    }

    #[test]
    fn chunk_zeros() {
        let mut reader = repeat(0u8);

        let poly = random_poly().unwrap();
        let rabin = Rabin64::new_with_polynom(6, poly);
        let mut chunker = ChunkIter::new(&mut reader, usize::MAX, rabin);

        let chunk = chunker.next().unwrap().unwrap();
        assert_eq!(constants::MIN_SIZE, chunk.len());
    }
}
