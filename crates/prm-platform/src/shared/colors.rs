//! Title-bar colors for DWM.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// `None` leaves the system default.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct TitleBarColors {
    pub dark: bool,
    pub caption: Option<Rgb>,
    pub text: Option<Rgb>,
    pub border: Option<Rgb>,
}

/// Win32 `COLORREF`: 0x00BBGGRR.
pub const fn colorref(c: Rgb) -> u32 {
    ((c.b as u32) << 16) | ((c.g as u32) << 8) | (c.r as u32)
}

#[cfg(test)]
mod tests;
