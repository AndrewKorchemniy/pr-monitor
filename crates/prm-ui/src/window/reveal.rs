//! Showing the window without a white flash: it stays DWM-cloaked from the moment it is shown
//! until a themed frame has been presented into it while on screen.

/// Where a cloaked window is on its way to being uncloaked.
///
/// Windows composes a window that has just been shown from a surface our renderer has not
/// presented into yet, which is white, and a present into a hidden window is not guaranteed to
/// survive the show. eframe also shows the window on its first frame before glow swaps buffers.
/// So the window is cloaked while hidden, shown cloaked, painted, and uncloaked only in the
/// `logic` after that paint (eframe presents between `ui` and the next `logic`).
///
/// Driven once per `logic` by [`Reveal::begin_logic`] and once per ui pass by
/// [`Reveal::painted`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reveal {
    /// Nothing to do: shown and uncloaked, or hidden (and possibly cloaked).
    Idle,
    /// Cloaked by the creator; eframe shows the window at the end of its first frame.
    Launching,
    /// Cloaked; the window is shown at the end of this frame, after this frame's paint.
    Showing,
    /// Cloaked and on screen; the next ui pass paints the first frame worth showing.
    AwaitPaint,
    /// That frame is presented after `ui` returns; uncloak in the next `logic`.
    AwaitPresent,
}

impl Reveal {
    /// Advances at the start of `logic`; true means uncloak now.
    pub(crate) const fn begin_logic(&mut self) -> bool {
        let (next, uncloak) = match *self {
            Self::Idle => (Self::Idle, false),
            Self::Launching => (Self::Showing, false),
            Self::Showing | Self::AwaitPaint => (Self::AwaitPaint, false),
            Self::AwaitPresent => (Self::Idle, true),
        };
        *self = next;
        uncloak
    }

    /// A ui pass ran with the window on screen.
    pub(crate) const fn painted(&mut self) {
        if matches!(self, Self::AwaitPaint) {
            *self = Self::AwaitPresent;
        }
    }

    /// The shown window is minimized (say, a shortcut set to "Run: Minimized"), so no ui pass
    /// will paint it. A minimized window shows nothing to flash, and a cloaked one must not be
    /// left that way. True means uncloak now.
    pub(crate) const fn minimized(&mut self) -> bool {
        match *self {
            Self::AwaitPaint | Self::AwaitPresent => {
                *self = Self::Idle;
                true
            }
            // why: before the show lands, `minimized` still describes the hidden window.
            Self::Idle | Self::Launching | Self::Showing => false,
        }
    }

    /// The next step only happens in another frame, so one must follow.
    pub(crate) const fn needs_frame(self) -> bool {
        matches!(self, Self::Showing | Self::AwaitPresent)
    }
}

#[cfg(test)]
mod tests;
