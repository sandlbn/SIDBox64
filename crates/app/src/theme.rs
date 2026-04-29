use iced::widget::button;
use iced::{Border, Color, Shadow};

pub const BACKGROUND: Color = Color {
    r: 0.051,
    g: 0.067,
    b: 0.090,
    a: 1.0,
}; // #0D1117
pub const PANEL: Color = Color {
    r: 0.086,
    g: 0.106,
    b: 0.133,
    a: 1.0,
}; // #161B22

pub const OFF_STEP: Color = Color {
    r: 0.188,
    g: 0.212,
    b: 0.239,
    a: 1.0,
}; // #30363D
pub const SOFT_STEP: Color = Color {
    r: 0.122,
    g: 0.306,
    b: 0.474,
    a: 1.0,
}; // #1F4E79
pub const MEDIUM_STEP: Color = Color {
    r: 0.176,
    g: 0.612,
    b: 0.859,
    a: 1.0,
}; // #2D9CDB
pub const LOUD_STEP: Color = Color {
    r: 0.898,
    g: 0.243,
    b: 0.243,
    a: 1.0,
}; // #E53E3E
pub const ACTIVE_STEP: Color = Color {
    r: 0.965,
    g: 0.753,
    b: 0.361,
    a: 1.0,
}; // #F6E05E
pub const BEAT_MARK: Color = Color {
    r: 0.165,
    g: 0.204,
    b: 0.255,
    a: 1.0,
}; // #2A3441

pub const TEXT: Color = Color {
    r: 0.788,
    g: 0.820,
    b: 0.851,
    a: 1.0,
}; // #C9D1D9
pub const ACCENT: Color = Color {
    r: 0.345,
    g: 0.651,
    b: 1.000,
    a: 1.0,
}; // #58A6FF
pub const DIM: Color = Color {
    r: 0.400,
    g: 0.450,
    b: 0.500,
    a: 1.0,
};

pub const BORDER_COLOR: Color = Color {
    r: 0.282,
    g: 0.329,
    b: 0.376,
    a: 1.0,
};

use drumbox64_core::Velocity;

/// Style for a step button.
pub fn step_style(
    vel: Velocity,
    is_active: bool,
    is_beat_start: bool,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let bg = if is_active {
            ACTIVE_STEP
        } else {
            let base = match vel {
                Velocity::Off => {
                    if is_beat_start {
                        BEAT_MARK
                    } else {
                        OFF_STEP
                    }
                }
                Velocity::Soft => SOFT_STEP,
                Velocity::Medium => MEDIUM_STEP,
                Velocity::Loud => LOUD_STEP,
            };
            match status {
                button::Status::Hovered => lighten(base, 0.08),
                button::Status::Pressed => lighten(base, 0.15),
                _ => base,
            }
        };

        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                color: if is_active { ACTIVE_STEP } else { BORDER_COLOR },
                width: if is_active { 2.0 } else { 1.0 },
                radius: 4.0.into(),
            },
            text_color: Color::TRANSPARENT,
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

/// Style for control buttons (Play, Stop, Save, Load).
pub fn control_style(highlight: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let base = if highlight { ACCENT } else { PANEL };
        let bg = match status {
            button::Status::Hovered => lighten(base, 0.10),
            button::Status::Pressed => lighten(base, 0.20),
            _ => base,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                color: BORDER_COLOR,
                width: 1.0,
                radius: 6.0.into(),
            },
            text_color: TEXT,
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

pub fn lighten(c: Color, amount: f32) -> Color {
    Color {
        r: (c.r + amount).min(1.0),
        g: (c.g + amount).min(1.0),
        b: (c.b + amount).min(1.0),
        a: c.a,
    }
}
