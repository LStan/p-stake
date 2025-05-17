#[repr(C)]
#[derive(PartialEq, Debug, Clone)]
pub struct StakeFlags {
    bits: u8,
}

impl StakeFlags {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn contains(&self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    pub fn remove(&mut self, other: Self) {
        self.bits &= !other.bits;
    }

    pub fn set(&mut self, other: Self) {
        self.bits |= other.bits;
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl Default for StakeFlags {
    fn default() -> Self {
        StakeFlags::empty()
    }
}
