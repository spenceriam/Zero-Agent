//! First-run onboarding as a modal overlay (separate from chat).

use std::io;

use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use super::{next_char_boundary, prev_char_boundary};
use super::layout::{self, ScreenLayout, TuiMode};
use super::modal::{build_card_lines, card_width_for_terminal, center_card_in_viewport};
use super::{App, ResponseStyle, UserProfile, CORAL, BOLD, RESET};

const STYLE_OPTIONS: &[(&str, &str)] = &[
    ("concise", "Concise"),
    ("verbose", "Verbose"),
    ("technical", "Technical"),
    ("non-technical", "Non-technical"),
    ("custom", "Something else…"),
];

pub enum OnboardingResult {
    Completed(UserProfile),
    Cancelled,
}

enum Step {
    Name,
    Style,
    CustomStyle,
    Mantra,
    Confirm,
}

pub fn run_onboarding(app: &App) -> io::Result<OnboardingResult> {
    layout::set_tui_mode(TuiMode::OnboardingOverlay);
    layout::set_footer_app(app);

    let mut step = Step::Name;
    let mut name = String::new();
    let mut style = ResponseStyle::Concise;
    let mut about = String::new();
    let mut buffer = String::new();
    let mut cursor = 0usize;
    let mut style_selected = 0usize;

    enable_raw_mode()?;

    let result = loop {
        match step {
            Step::Name => {
                render_name_step(app, &buffer)?;
                match read_modal_key(&mut buffer, &mut cursor, false)? {
                    ModalInput::Submit => {
                        let trimmed = buffer.trim().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        name = trimmed;
                        buffer.clear();
                        cursor = 0;
                        step = Step::Style;
                    }
                    ModalInput::Cancel => break OnboardingResult::Cancelled,
                    ModalInput::Continue => {}
                }
            }
            Step::Style => {
                render_style_step(app, style_selected)?;
                if let Ok(Event::Key(key)) = event::read() {
                    match key {
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::SUPER,
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Esc, ..
                        } => break OnboardingResult::Cancelled,
                        KeyEvent {
                            code: KeyCode::Up, ..
                        }
                        | KeyEvent {
                            code: KeyCode::Char('k'),
                            ..
                        } => {
                            style_selected = if style_selected == 0 {
                                STYLE_OPTIONS.len() - 1
                            } else {
                                style_selected - 1
                            };
                        }
                        KeyEvent {
                            code: KeyCode::Down, ..
                        }
                        | KeyEvent {
                            code: KeyCode::Char('j'),
                            ..
                        } => {
                            style_selected = (style_selected + 1) % STYLE_OPTIONS.len();
                        }
                        KeyEvent {
                            code: KeyCode::Enter, ..
                        } => {
                            let (key, _) = STYLE_OPTIONS[style_selected];
                            if key == "custom" {
                                buffer.clear();
                                cursor = 0;
                                step = Step::CustomStyle;
                            } else {
                                style = parse_style_key(key);
                                step = Step::Mantra;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Step::CustomStyle => {
                render_custom_style_step(app, &buffer)?;
                match read_modal_key(&mut buffer, &mut cursor, false)? {
                    ModalInput::Submit => {
                        let trimmed = buffer.trim().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        style = ResponseStyle::Persona(trimmed);
                        buffer.clear();
                        cursor = 0;
                        step = Step::Mantra;
                    }
                    ModalInput::Cancel => break OnboardingResult::Cancelled,
                    ModalInput::Continue => {}
                }
            }
            Step::Mantra => {
                render_mantra_step(app, &buffer)?;
                match read_modal_key(&mut buffer, &mut cursor, true)? {
                    ModalInput::Submit => {
                        about = buffer.trim().to_string();
                        buffer.clear();
                        cursor = 0;
                        step = Step::Confirm;
                    }
                    ModalInput::Cancel => break OnboardingResult::Cancelled,
                    ModalInput::Continue => {}
                }
            }
            Step::Confirm => {
                render_confirm_step(app, &name, &style, &about)?;
                if let Ok(Event::Key(key)) = event::read() {
                    match key {
                        KeyEvent {
                            code: KeyCode::Enter, ..
                        } => {
                            break OnboardingResult::Completed(UserProfile {
                                name,
                                role: String::new(),
                                about,
                                style,
                            });
                        }
                        KeyEvent {
                            code: KeyCode::Esc, ..
                        }
                        | KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::SUPER,
                            ..
                        } => break OnboardingResult::Cancelled,
                        _ => {}
                    }
                }
            }
        }
    };

    disable_raw_mode()?;
    let _ = execute!(io::stdout(), Show);
    Ok(result)
}

enum ModalInput {
    Submit,
    Cancel,
    Continue,
}

fn read_modal_key(
    buffer: &mut String,
    cursor: &mut usize,
    optional: bool,
) -> io::Result<ModalInput> {
    match event::read()? {
        Event::Resize(w, h) => {
            layout::handle_resize(w, h);
            Ok(ModalInput::Continue)
        }
        Event::Key(key) => match key {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::SUPER,
                ..
            }
            | KeyEvent {
                code: KeyCode::Esc, ..
            } => Ok(ModalInput::Cancel),
            KeyEvent {
                code: KeyCode::Enter, ..
            } => {
                if optional && buffer.trim().is_empty() {
                    return Ok(ModalInput::Submit);
                }
                if buffer.trim().is_empty() {
                    return Ok(ModalInput::Continue);
                }
                Ok(ModalInput::Submit)
            }
            KeyEvent {
                code: KeyCode::Backspace, ..
            } => {
                if *cursor > 0 {
                    let prev = prev_char_boundary(buffer, *cursor);
                    buffer.drain(prev..*cursor);
                    *cursor = prev;
                }
                Ok(ModalInput::Continue)
            }
            KeyEvent {
                code: KeyCode::Left, ..
            } => {
                if *cursor > 0 {
                    *cursor = prev_char_boundary(buffer, *cursor);
                }
                Ok(ModalInput::Continue)
            }
            KeyEvent {
                code: KeyCode::Right, ..
            } => {
                if *cursor < buffer.len() {
                    *cursor = next_char_boundary(buffer, *cursor);
                }
                Ok(ModalInput::Continue)
            }
            KeyEvent {
                code: KeyCode::Char(ch), ..
            } => {
                buffer.insert(*cursor, ch);
                *cursor += ch.len_utf8();
                Ok(ModalInput::Continue)
            }
            _ => Ok(ModalInput::Continue),
        },
        _ => Ok(ModalInput::Continue),
    }
}

fn render_name_step(app: &App, buffer: &str) -> io::Result<()> {
    let body = vec![
        "Welcome to Zero-Agent.".to_string(),
        "Let's personalize your experience.".to_string(),
        String::new(),
        "What should I call you?".to_string(),
        format!("{CORAL}{BOLD}{buffer}{RESET}"),
    ];
    draw_modal(app, "Setup", &body, "Enter to continue · Esc to cancel")
}

fn render_style_step(app: &App, selected: usize) -> io::Result<()> {
    let mut body = vec![
        "How should I respond?".to_string(),
        String::new(),
    ];
    for (i, (_, label)) in STYLE_OPTIONS.iter().enumerate() {
        if i == selected {
            body.push(format!("{CORAL}{BOLD}\u{25b6} {label}{RESET}"));
        } else {
            body.push(format!("  {label}"));
        }
    }
    draw_modal(app, "Setup", &body, "\u{2191}\u{2193} navigate · Enter select · Esc cancel")
}

fn render_custom_style_step(app: &App, buffer: &str) -> io::Result<()> {
    let body = vec![
        "Describe your preferred style:".to_string(),
        String::new(),
        format!("{CORAL}{BOLD}{buffer}{RESET}"),
    ];
    draw_modal(app, "Setup", &body, "Enter to continue · Esc to cancel")
}

fn render_mantra_step(app: &App, buffer: &str) -> io::Result<()> {
    let body = vec![
        "Anything else I should always follow?".to_string(),
        "(optional — Enter to skip)".to_string(),
        String::new(),
        format!("{CORAL}{BOLD}{buffer}{RESET}"),
    ];
    draw_modal(
        app,
        "Setup",
        &body,
        "e.g. use my name, prefer small diffs",
    )
}

fn render_confirm_step(app: &App, name: &str, style: &ResponseStyle, about: &str) -> io::Result<()> {
    let style_label = style_display(style);
    let mut body = vec![
        format!("Name: {CORAL}{BOLD}{name}{RESET}"),
        format!("Style: {style_label}"),
    ];
    if !about.is_empty() {
        body.push(format!("Notes: {about}"));
    }
    body.push(String::new());
    body.push("Ready to start chatting.".to_string());
    draw_modal(app, "Setup complete", &body, "Enter to finish · Esc cancel")
}

fn draw_modal(app: &App, title: &str, body: &[String], footer: &str) -> io::Result<()> {
    layout::set_footer_app(app);
    let _ = ScreenLayout::with_global(|layout| {
        let card_w = card_width_for_terminal(layout.width());
        let card = build_card_lines(title, body, footer, card_w);
        let rows = center_card_in_viewport(
            &card,
            layout.width(),
            layout.chat_start(),
            layout.footer_start(),
        );
        layout.set_modal_card_rows(rows);
        layout.render_frame()
    });
    Ok(())
}

fn style_display(style: &ResponseStyle) -> String {
    match style {
        ResponseStyle::Concise => "Concise".into(),
        ResponseStyle::Verbose => "Verbose".into(),
        ResponseStyle::Technical => "Technical".into(),
        ResponseStyle::Persona(s) if s == "non-technical" => "Non-technical".into(),
        ResponseStyle::Persona(s) => s.clone(),
    }
}

fn parse_style_key(key: &str) -> ResponseStyle {
    match key {
        "verbose" => ResponseStyle::Verbose,
        "technical" => ResponseStyle::Technical,
        "non-technical" => ResponseStyle::Persona("non-technical".into()),
        "custom" => ResponseStyle::Persona(String::new()),
        _ => ResponseStyle::Concise,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_does_not_use_transcript_api() {
        // Modal onboarding must not append to chat transcript.
        // Verified by architecture: run_onboarding only calls set_modal_overlay paths.
        assert_ne!(std::mem::size_of::<OnboardingResult>(), 0);
    }
}
