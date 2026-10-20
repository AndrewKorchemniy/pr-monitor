//! FNV-1a 64-bit. Stable across Rust versions and platforms, so its output may be persisted
//! (fingerprints in `state.json`, toast tags). Not a cryptographic hash.

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv64 {
    pub const fn new() -> Self {
        Self(OFFSET)
    }

    pub fn write_bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    /// Length-prefixed, so `("ab","c")` and `("a","bc")` hash differently.
    pub fn write_str(&mut self, s: &str) {
        self.write_u64(s.len() as u64);
        self.write_bytes(s.as_bytes());
    }

    pub fn write_opt_str(&mut self, s: Option<&str>) {
        match s {
            Some(s) => {
                self.write_u8(1);
                self.write_str(s);
            }
            None => self.write_u8(0),
        }
    }

    pub fn write_u8(&mut self, v: u8) {
        self.write_bytes(&[v]);
    }

    pub fn write_bool(&mut self, v: bool) {
        self.write_u8(u8::from(v));
    }

    pub fn write_i16(&mut self, v: i16) {
        self.write_bytes(&v.to_le_bytes());
    }

    pub fn write_i32(&mut self, v: i32) {
        self.write_bytes(&v.to_le_bytes());
    }

    pub fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes());
    }

    pub fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    pub const fn finish(self) -> u64 {
        self.0
    }
}

pub fn fnv64(b: &[u8]) -> u64 {
    let mut h = Fnv64::new();
    h.write_bytes(b);
    h.finish()
}

#[cfg(test)]
mod tests;
