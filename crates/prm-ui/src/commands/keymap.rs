//! Keyboard shortcuts: defaults from the registry, user overrides from `keymap.toml`, conflict
//! reporting, and matching against egui input per scope.

use std::{collections::BTreeMap, fmt, str::FromStr};

use eframe::egui::{self, Key, Modifiers};

use crate::commands::{COMMANDS, CommandId, Scope};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct KeyChord {
    /// Ctrl on Windows/Linux, Cmd on macOS.
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChordError(pub String);

impl FromStr for KeyChord {
    type Err = ChordError;

    /// `"Ctrl+Shift+C"`, `"F5"`, `"Space"`, `"/"`, `"Ctrl+Comma"`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ChordError(s.to_owned());
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let (key_name, mods) = parts.split_last().ok_or_else(invalid)?;
        let mut chord = Self {
            ctrl: false,
            shift: false,
            alt: false,
            key: Key::from_name(key_name).ok_or_else(invalid)?,
        };
        for modifier in mods {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" | "command" => chord.ctrl = true,
                "shift" => chord.shift = true,
                "alt" | "option" => chord.alt = true,
                _ => return Err(invalid()),
            }
        }
        Ok(chord)
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        f.write_str(key_label(self.key))
    }
}

/// Short labels for hints ("Down", ",", "Space"); the UI font has no arrow glyphs.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "every other key uses egui's own name"
)]
fn key_label(key: Key) -> &'static str {
    match key {
        Key::ArrowDown => "Down",
        Key::ArrowUp => "Up",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::Space => "Space",
        other => other.symbol_or_name(),
    }
}

impl KeyChord {
    const fn modifier_count(self) -> u8 {
        self.ctrl as u8 + self.shift as u8 + self.alt as u8
    }

    fn modifiers(self) -> Modifiers {
        let mut modifiers = Modifiers::NONE;
        if self.ctrl {
            modifiers |= Modifiers::COMMAND;
        }
        if self.shift {
            modifiers |= Modifiers::SHIFT;
        }
        if self.alt {
            modifiers |= Modifiers::ALT;
        }
        modifiers
    }

    /// egui turns Ctrl+C into `Event::Copy` (clipboard shortcuts), never a key event.
    fn is_copy(self) -> bool {
        self.ctrl && !self.shift && !self.alt && self.key == Key::C
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    pub chord: KeyChord,
    pub command: CommandId,
    pub scope: Scope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeymapIssue(pub String);

/// Which bindings may fire right now.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ActiveScope {
    /// The palette or a modal owns the keyboard.
    Overlay,
    /// A text field has focus: only global shortcuts.
    TextInput,
    Table,
    Settings,
}

impl ActiveScope {
    const fn allows(self, scope: Scope) -> bool {
        match (self, scope) {
            (Self::Overlay, _) => false,
            (Self::TextInput | Self::Table | Self::Settings, Scope::Global)
            | (Self::Table, Scope::Table)
            | (Self::Settings, Scope::Settings) => true,
            (Self::TextInput, Scope::Table | Scope::Settings)
            | (Self::Table, Scope::Settings)
            | (Self::Settings, Scope::Table) => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Keymap {
    bindings: Vec<Binding>,
}

impl Keymap {
    pub(crate) fn defaults() -> Self {
        let bindings = COMMANDS
            .iter()
            .flat_map(|spec| {
                spec.keys.iter().filter_map(|(chord, scope)| {
                    Some(Binding {
                        chord: chord.parse().ok()?,
                        command: spec.id,
                        scope: *scope,
                    })
                })
            })
            .collect();
        Self { bindings }
    }

    /// `[bindings] "refresh.now" = ["F5"]`; an empty list unbinds.
    pub(crate) fn with_overrides(mut self, toml_src: &str) -> (Self, Vec<KeymapIssue>) {
        #[derive(serde::Deserialize, Default)]
        #[serde(default)]
        struct File {
            bindings: BTreeMap<String, Vec<String>>,
        }
        let mut issues = Vec::new();
        let file: File = match toml::from_str(toml_src) {
            Ok(file) => file,
            Err(error) => {
                issues.push(KeymapIssue(format!("keymap.toml: {}", error.message())));
                return (self, issues);
            }
        };
        for (key, chords) in file.bindings {
            let Some(command) = CommandId::from_key(&key) else {
                issues.push(KeymapIssue(format!("keymap.toml: unknown command “{key}”")));
                continue;
            };
            let scope = command
                .spec()
                .keys
                .first()
                .map_or(Scope::Global, |(_, s)| *s);
            self.bindings.retain(|b| b.command != command);
            let (good, bad): (Vec<_>, Vec<_>) = chords
                .into_iter()
                .partition(|t| t.parse::<KeyChord>().is_ok());
            let parsed = good.iter().filter_map(|t| t.parse().ok());
            self.bindings.extend(parsed.map(|chord| Binding {
                chord,
                command,
                scope,
            }));
            issues.extend(
                bad.iter()
                    .map(|t| KeymapIssue(format!("keymap.toml: “{t}” is not a key"))),
            );
        }
        for (a, _) in self.conflicts() {
            issues.push(KeymapIssue(format!(
                "keymap.toml: {} is used twice",
                a.chord
            )));
        }
        (self, issues)
    }

    /// Bindings sharing a chord in overlapping scopes.
    pub(crate) fn conflicts(&self) -> Vec<(&Binding, &Binding)> {
        let overlaps = |a: Scope, b: Scope| a == b || a == Scope::Global || b == Scope::Global;
        let clash = |a: &Binding, b: &Binding| {
            a.chord == b.chord && a.command != b.command && overlaps(a.scope, b.scope)
        };
        self.bindings
            .iter()
            .enumerate()
            .flat_map(|(i, a)| self.bindings[i + 1..].iter().map(move |b| (a, b)))
            .filter(|(a, b)| clash(a, b))
            .collect()
    }

    /// The first chord bound to a command (for hints and the shortcut sheet).
    pub(crate) fn hint(&self, command: CommandId) -> Option<KeyChord> {
        self.bindings
            .iter()
            .find(|b| b.command == command)
            .map(|b| b.chord)
    }

    pub(crate) fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Consumes matched input and returns the commands in order. Navigation keys repeat.
    pub(crate) fn dispatch(&self, ctx: &egui::Context, scope: ActiveScope) -> Vec<CommandId> {
        let mut active: Vec<&Binding> = self
            .bindings
            .iter()
            .filter(|b| scope.allows(b.scope))
            .collect();
        // why: logical matching ignores an unrequested Shift, so "Shift+M" must be tried before "M".
        active.sort_by_key(|b| std::cmp::Reverse(b.chord.modifier_count()));
        ctx.input_mut(|input| {
            active
                .iter()
                .flat_map(|binding| fire(input, **binding))
                .collect()
        })
    }
}

/// The binding's command once per matching key press (consumed).
fn fire(input: &mut egui::InputState, binding: Binding) -> std::iter::RepeatN<CommandId> {
    let count = if binding.chord.is_copy() {
        usize::from(consume_copy(input))
    } else {
        input.count_and_consume_key(binding.chord.modifiers(), binding.chord.key)
    };
    std::iter::repeat_n(binding.command, count)
}

/// Removes `Event::Copy` (egui turns Ctrl+C into it); true when there was one.
fn consume_copy(input: &mut egui::InputState) -> bool {
    let before = input.events.len();
    input.events.retain(|e| !matches!(e, egui::Event::Copy));
    input.events.len() != before
}

#[cfg(test)]
mod tests;
