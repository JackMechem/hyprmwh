use iced::widget::{button, container};
use iced::Color;

use crate::app::App;
use crate::config::{get, parse_color};

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

pub fn button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let s = &get().style;
    button::Style {
        background: Some(iced::Background::Color(match status {
            button::Status::Hovered => parse_color(&s.button_hover),
            button::Status::Pressed => parse_color(&s.button_pressed),
            _ => parse_color(&s.button_background),
        })),
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: s.button_radius.into(),
        },
        text_color: parse_color(&s.text_color),
        ..Default::default()
    }
}

pub fn button_style_selected(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let s = &get().style;
    button::Style {
        background: Some(iced::Background::Color(match status {
            button::Status::Pressed => parse_color(&s.button_selected_hover),
            _ => parse_color(&s.button_selected_background),
        })),
        border: iced::Border {
            color: parse_color(&s.button_selected_border),
            width: 1.0,
            radius: s.button_radius.into(),
        },
        text_color: parse_color(&s.text_color),
        ..Default::default()
    }
}

pub fn statusbar_style(_theme: &iced::Theme) -> container::Style {
    let s = &get().style;
    container::Style {
        background: Some(iced::Background::Color(parse_color(&s.statusbar_background))),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
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
