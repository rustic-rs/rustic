// The irreductible polynom to be used in the fingerprint function.
pub trait Polynom {
    fn degree(&self) -> i32;
    fn modulo(&self, m: &Self) -> Self;
}

pub type Polynom64 = u64;

impl Polynom for Polynom64 {
    // The degree of the polynom.
    fn degree(&self) -> i32 {
        63 - self.leading_zeros() as i32
    }

    fn modulo(&self, m: &Self) -> Self {
        let mut p = *self;
        while p.degree() >= m.degree() {
            p ^= m << (p.degree() - m.degree());
        }

        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynom_degree() {
        assert_eq!(0u64.degree(), -1);
        assert_eq!(1u64.degree(), 0);

        assert_eq!(((1u64 << 7) - 1).degree(), 6);
        assert_eq!((1u64 << 7).degree(), 7);
        assert_eq!(((1u64 << 7) + 1).degree(), 7);
    }

    #[test]
    fn polynom_modulo() {
        assert_eq!(7u64.modulo(&3), 1);
        assert_eq!(7u64.modulo(&4), 3);
        assert_eq!(7u64.modulo(&2), 1);

        assert_eq!(16u64.modulo(&8), 0);
        assert_eq!(19u64.modulo(&8), 3);

        assert_eq!(16u64.modulo(&4), 0);
        assert_eq!(19u64.modulo(&4), 3);
    }
}
