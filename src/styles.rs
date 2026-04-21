use iced::widget::{container, scrollable};
use iced::Color;

use crate::app::App;
use crate::config::{get, parse_color};

pub fn scrollbar_style(_theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
    let s = &get().style;
    let scroller_color = match status {
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => {
            parse_color(&s.statusbar_text)
        }
        _ => {
            // Dimmer when idle
            let mut c = parse_color(&s.statusbar_text);
            c.a *= 0.4;
            c
        }
    };

    let rail = scrollable::Rail {
        background: None,
        border: iced::Border::default(),
        scroller: scrollable::Scroller {
            background: iced::Background::Color(scroller_color),
            border: iced::Border {
                radius: 2.0.into(),
                ..Default::default()
            },
        },
    };

    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        ..scrollable::default(_theme, status)
    }
}

pub fn container_style(_theme: &iced::Theme) -> container::Style {
    let s = &get().style;
    container::Style {
        background: Some(iced::Background::Color(parse_color(&s.container_background))),
        border: iced::Border {
            color: parse_color(&s.container_border),
            width: 1.0,
            radius: iced::border::Radius {
                top_left: s.container_radius,
                top_right: s.container_radius,
                bottom_left: s.container_radius,
                bottom_right: s.container_radius,
            },
        },
        ..Default::default()
    }
}

pub fn window_style(_: &App, _theme: &iced::Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: parse_color(&get().style.text_color),
    }
}
