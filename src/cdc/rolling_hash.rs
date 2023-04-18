use super::{Polynom, Polynom64};

pub trait RollingHash64 {
    fn reset(&mut self);
    fn prefill_window<I>(&mut self, iter: &mut I) -> usize
    where
        I: Iterator<Item = u8>;
    fn reset_and_prefill_window<I>(&mut self, iter: &mut I) -> usize
    where
        I: Iterator<Item = u8>;
    fn slide(&mut self, byte: u8);
    fn get_hash(&self) -> &Polynom64;
}

#[derive(Clone)]
pub struct Rabin64 {
    // Configuration
    window_size: usize, // The size of the data window used in the hash calculation.
    window_size_mask: usize, // = window_size - 1, supposing that it is an exponent of 2.

    // Precalculations
    polynom_shift: i32,
    out_table: [Polynom64; 256],
    mod_table: [Polynom64; 256],

    // Current state
    window_data: Vec<u8>,
    window_index: usize,
    pub hash: Polynom64,
}

impl Rabin64 {
    pub fn calculate_out_table(window_size: usize, mod_polynom: Polynom64) -> [Polynom64; 256] {
        let mut out_table = [0; 256];
        for (b, elem) in out_table.iter_mut().enumerate() {
            let mut hash = (b as Polynom64).modulo(mod_polynom);
            for _ in 0..window_size - 1 {
                hash <<= 8;
                hash = hash.modulo(mod_polynom);
            }
            *elem = hash;
        }

        out_table
    }

    pub fn calculate_mod_table(mod_polynom: Polynom64) -> [Polynom64; 256] {
        let mut mod_table = [0; 256];
        let k = mod_polynom.degree();
        for (b, elem) in mod_table.iter_mut().enumerate() {
            let p: Polynom64 = (b as Polynom64) << k;
            *elem = p.modulo(mod_polynom) | p;
        }

        mod_table
    }

    pub fn new_with_polynom(window_size_nb_bits: u32, mod_polynom: Polynom64) -> Rabin64 {
        let window_size = 1 << window_size_nb_bits;

        let window_data = vec![0; window_size];

        Rabin64 {
            window_size,
            window_size_mask: window_size - 1,
            polynom_shift: mod_polynom.degree() - 8,
            out_table: Self::calculate_out_table(window_size, mod_polynom),
            mod_table: Self::calculate_mod_table(mod_polynom),
            window_data,
            window_index: 0,
            hash: 0,
        }
    }
}

impl RollingHash64 for Rabin64 {
    fn reset(&mut self) {
        self.window_data.clear();
        self.window_data.resize(self.window_size, 0);
        self.window_index = 0;
        self.hash = 0;

        // Not needed.
        // self.slide(1);
    }

    // Attempt to fills the window - 1 byte.
    fn prefill_window<I>(&mut self, iter: &mut I) -> usize
    where
        I: Iterator<Item = u8>,
    {
        let mut nb_bytes_read = 0;
        for _ in 0..self.window_size - 1 {
            match iter.next() {
                Some(b) => {
                    self.slide(b);
                    nb_bytes_read += 1;
                }
                None => break,
            }
        }

        nb_bytes_read
    }

    // Combines a reset with a prefill in an optimized way.
    fn reset_and_prefill_window<I>(&mut self, iter: &mut I) -> usize
    where
        I: Iterator<Item = u8>,
    {
        self.hash = 0;
        let mut nb_bytes_read = 0;
        for _ in 0..self.window_size - 1 {
            match iter.next() {
                Some(b) => {
                    // Take the old value out of the window and the hash.
                    // ... let's suppose that the buffer contains zeroes, do nothing.

                    // Put the new value in the window and in the hash.
                    self.window_data[self.window_index] = b;
                    let mod_index = (self.hash >> self.polynom_shift) & 255;
                    self.hash <<= 8;
                    self.hash |= u64::from(b);
                    self.hash ^= self.mod_table[mod_index as usize];

                    // Move the windowIndex to the next position.
                    self.window_index = (self.window_index + 1) & self.window_size_mask;

                    nb_bytes_read += 1;
                }
                None => break,
            }
        }

        // Because we didn't overwrite that element in the loop above.
        self.window_data[self.window_index] = 0;

        nb_bytes_read
    }

    #[inline]
    fn slide(&mut self, byte: u8) {
        // Take the old value out of the window and the hash.
        let out_value = self.window_data[self.window_index];
        self.hash ^= self.out_table[out_value as usize];

        // Put the new value in the window and in the hash.
        self.window_data[self.window_index] = byte;
        let mod_index = (self.hash >> self.polynom_shift) & 255;
        self.hash <<= 8;
        self.hash |= u64::from(byte);
        self.hash ^= self.mod_table[mod_index as usize];

        // Move the windowIndex to the next position.
        self.window_index = (self.window_index + 1) & self.window_size_mask;
    }

    #[inline]
    fn get_hash(&self) -> &Polynom64 {
        &self.hash
    }
}
