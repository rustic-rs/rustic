use crate::cdc::polynom::{Polynom, Polynom64};

/// A rolling hash implementataion for 64 bit polynoms.
pub(crate) trait RollingHash64 {
    /// Resets the rolling hash.
    fn reset(&mut self);

    /// Attempt to prefill the window
    ///
    /// # Arguments
    ///
    /// * `iter` - The iterator to read from.
    fn prefill_window<I>(&mut self, iter: &mut I) -> usize
    where
        I: Iterator<Item = u8>;

    /// Combines a reset with a prefill in an optimized way.
    ///
    /// # Arguments
    ///
    /// * `iter` - The iterator to read from.
    fn reset_and_prefill_window<I>(&mut self, iter: &mut I) -> usize
    where
        I: Iterator<Item = u8>;

    /// Slides the window by byte.
    ///
    /// # Arguments
    ///
    /// * `byte` - The byte to slide in.
    fn slide(&mut self, byte: u8);

    /// Returns the current hash as a `Polynom64`.
    fn get_hash(&self) -> &Polynom64;
}

/// A rolling hash implementataion for 64 bit polynoms from Rabin.
#[derive(Clone)]
pub(crate) struct Rabin64 {
    // Configuration
    /// Window size.
    pub(crate) window_size: usize, // The size of the data window used in the hash calculation.
    /// Window size mask.
    pub(crate) window_size_mask: usize, // = window_size - 1, supposing that it is an exponent of 2.

    // Precalculations
    /// The number of bits to shift the polynom to the left.
    pub(crate) polynom_shift: i32,

    /// Precalculated out table.
    pub(crate) out_table: [Polynom64; 256],
    /// Precalculated mod table.
    pub(crate) mod_table: [Polynom64; 256],

    // Current state
    /// The data window.
    pub(crate) window_data: Vec<u8>,
    /// The current window index.
    pub(crate) window_index: usize,
    /// The current hash.
    pub(crate) hash: Polynom64,
}

impl Rabin64 {
    /// Calculates the out table
    ///
    /// # Arguments
    ///
    /// * `window_size` - The window size.
    /// * `mod_polynom` - The modulo polynom.
    ///
    /// # Returns
    ///
    /// An array of 256 `Polynom64` values.
    fn calculate_out_table(window_size: usize, mod_polynom: Polynom64) -> [Polynom64; 256] {
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

    /// Calculates the mod table
    ///
    /// # Arguments
    ///
    /// * `mod_polynom` - The modulo polynom.
    ///
    /// # Returns
    ///
    /// An array of 256 `Polynom64` values.
    fn calculate_mod_table(mod_polynom: Polynom64) -> [Polynom64; 256] {
        let mut mod_table = [0; 256];
        let k = mod_polynom.degree();
        for (b, elem) in mod_table.iter_mut().enumerate() {
            let p: Polynom64 = (b as Polynom64) << k;
            *elem = p.modulo(mod_polynom) | p;
        }

        mod_table
    }

    /// Creates a new `Rabin64` with the given window size and modulo polynom.
    ///
    /// # Arguments
    ///
    /// * `window_size_nb_bits` - The number of bits of the window size.
    /// * `mod_polynom` - The modulo polynom.
    pub(crate) fn new_with_polynom(window_size_nb_bits: u32, mod_polynom: Polynom64) -> Self {
        let window_size = 1 << window_size_nb_bits;

        let window_data = vec![0; window_size];

        Self {
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
