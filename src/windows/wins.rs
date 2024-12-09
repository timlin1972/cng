use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEvent},
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::windows::{win_error, win_info, win_main};

#[derive(PartialEq)]
pub enum RetKey {
    RKLeave,
    RKContinue,
    RKCommand(String),
}

pub trait Window {
    fn title(&self) -> &str;
    fn input(&self) -> &str;
    fn output(&self) -> &Vec<String>;
    fn output_push(&mut self, output: String);
    fn key(&mut self, key: KeyEvent) -> RetKey;
}

pub struct Windows {
    pub windows: Vec<Box<dyn Window>>,
    active_window: usize,
}

impl Windows {
    pub fn new() -> Self {
        let windows = vec![
            Box::new(win_main::Window::new("main".to_string())) as Box<dyn Window>,
            Box::new(win_info::Window::new("info".to_string())) as Box<dyn Window>,
            Box::new(win_error::Window::new("error".to_string())) as Box<dyn Window>,
        ];

        Self {
            windows,
            active_window: 0,
        }
    }

    pub fn next_window(&mut self) {
        self.active_window = (self.active_window + 1) % self.windows.len();
    }

    pub fn draw(&self, frame: &mut Frame) {
        // layout allocation
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(90), Constraint::Length(1)])
            .split(frame.area());

        let [area_top, area_command] = [layout[0], layout[1]];

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area_top);

        let [area_main, area_right] = [layout[0], layout[1]];

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area_right);

        let [area_info, area_error] = [layout[0], layout[1]];

        for (index, window) in self.windows.iter().enumerate() {
            if window.title() == "command" {
                continue;
            }

            let block = Block::default()
                .title(window.title())
                .borders(Borders::ALL)
                .border_type(if index == self.active_window {
                    BorderType::Double
                } else {
                    BorderType::Plain
                })
                .style(if index == self.active_window {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                });

            let area_height = match window.title() {
                "main" => area_main.height,
                "info" => area_info.height,
                "error" => area_error.height,
                _ => panic!(),
            };

            let scroll_offset = if window.output().len() as u16 > (area_height - 2) {
                window.output().len() as u16 - (area_height - 2)
            } else {
                0
            };

            let paragraph = Paragraph::new(window.output().join("\n"))
                .block(block)
                .scroll((scroll_offset, 0));
            frame.render_widget(
                paragraph,
                match window.title() {
                    "main" => area_main,
                    "info" => area_info,
                    "error" => area_error,
                    _ => panic!(),
                },
            );
        }

        let input = self.windows.get(self.active_window).unwrap().input();

        let paragraph_command = Paragraph::new(format!("> {}", input)).style(Style::default());
        frame.render_widget(paragraph_command, area_command);

        frame.set_cursor_position(Position::new(
            area_command.x + input.len() as u16 + 2,
            area_command.y,
        ));
    }

    pub fn key(&mut self) -> RetKey {
        let mut ret = RetKey::RKContinue;
        if let Event::Key(key) = event::read().unwrap() {
            match key.code {
                KeyCode::Tab => {
                    self.next_window();
                }
                _ => {
                    ret = self.windows.get_mut(self.active_window).unwrap().key(key);
                }
            }
        }

        ret
    }
}
