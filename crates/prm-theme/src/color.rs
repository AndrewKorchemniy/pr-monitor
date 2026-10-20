//! `Rgba8`: an sRGB color with straight alpha, written as `#RRGGBB` or `#RRGGBBAA`.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a color; use #RRGGBB or #RRGGBBAA")]
pub struct ColorError(pub String);

impl Rgba8 {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// `#RRGGBB`, or `#RRGGBBAA` when not opaque.
    pub fn to_hex(self) -> String {
        if self.a == u8::MAX {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }
}

impl fmt::Display for Rgba8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl FromStr for Rgba8 {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ColorError(s.to_owned());
        let hex = s.trim().strip_prefix('#').ok_or_else(invalid)?;
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| invalid());
        let a = if hex.len() == 8 { channel(6)? } else { u8::MAX };
        Ok(Self {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
            a,
        })
    }
}

impl TryFrom<String> for Rgba8 {
    type Error = ColorError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Rgba8> for String {
    fn from(color: Rgba8) -> Self {
        color.to_hex()
    }
}

#[cfg(test)]
mod tests;
