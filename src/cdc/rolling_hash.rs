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
    pub fn calculate_out_table(window_size: usize, mod_polynom: &Polynom64) -> [Polynom64; 256] {
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

    pub fn calculate_mod_table(mod_polynom: &Polynom64) -> [Polynom64; 256] {
        let mut mod_table = [0; 256];
        let k = mod_polynom.degree();
        for (b, elem) in mod_table.iter_mut().enumerate() {
            let p: Polynom64 = (b as Polynom64) << k;
            *elem = p.modulo(mod_polynom) | p;
        }

        mod_table
    }

    pub fn new_with_polynom(window_size_nb_bits: u32, mod_polynom: &Polynom64) -> Rabin64 {
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

    #[cfg(test)]
    pub fn hash_block(&mut self, bytes: &[u8], mod_polynom: &Polynom64) {
        for v in bytes {
            self.hash <<= 8;
            self.hash |= *v as Polynom64;
            self.hash = self.hash.modulo(&mod_polynom);
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
                    self.hash |= b as Polynom64;
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
        self.hash |= byte as Polynom64;
        self.hash ^= self.mod_table[mod_index as usize];

        // Move the windowIndex to the next position.
        self.window_index = (self.window_index + 1) & self.window_size_mask;
    }

    #[inline]
    fn get_hash(&self) -> &Polynom64 {
        &self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::super::polynom::Polynom64;
    use super::*;

    fn to_hex_string(polynoms: &[Polynom64], prefix: &str) -> String {
        let strs: Vec<String> = polynoms
            .iter()
            .map(|p| format!("{}{:016x} {}", prefix, p, 0))
            .collect();
        strs.join("\n")
    }

    #[test]
    fn print_tables() {
        let out_table = Rabin64::calculate_out_table(32, &MOD_POLYNOM);
        let mod_table = Rabin64::calculate_mod_table(&MOD_POLYNOM);
        println!("{}", to_hex_string(&out_table[..], "outTable "));
        println!("{}", to_hex_string(&mod_table[..], "modTable "));
    }

    #[test]
    fn rabin_hash() {
        use std::cmp::max;

        // Random meaningless data.
        let data = [
            17u8, 28, 53, 64, 175, 216, 27, 208, 109, 130, 143, 35, 93, 244, 45, 18, 64, 193, 204,
            59, 169, 139, 53, 59, 55, 65, 242, 73, 60, 198, 45, 22, 56, 90, 81, 181,
        ];

        let mut rabin1 = Rabin64::new(5);
        let mut rabin2 = Rabin64::new(5);

        // Block by block, no optimization, used raw modulo formula.
        for i in 0..data.len() {
            let block = &data[max(31, i) - 31..i + 1];
            rabin1.reset();
            rabin1.hash_block(block, &MOD_POLYNOM);

            rabin2.slide(data[i]);

            //println!("{:02} {:02} {:016x} {:016x} {:?}", i, block.len(), rabin1.hash, rabin2.hash, block);
            assert_eq!(rabin1.hash, rabin2.hash);
        }
    }
}
