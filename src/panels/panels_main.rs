use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEvent},
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::panels::{panel_error, panel_info, panel_brief};

#[derive(PartialEq)]
pub enum RetKey {
    RKLeave,
    RKContinue,
}

pub trait Panel {
    fn title(&self) -> &str;
    fn input(&self) -> &str;
    fn output(&self) -> &Vec<String>;
    fn output_push(&mut self, output: String);
    fn key(&mut self, key: KeyEvent) -> RetKey;
    fn run(&mut self, _cmd: &str) -> RetKey {
        RetKey::RKContinue
    }
}

pub struct Panels {
    pub panels: Vec<Box<dyn Panel>>,
    active_panel: usize,
}

impl Panels {
    pub fn new() -> Self {
        let panels = vec![
            Box::new(panel_brief::Panel::new()) as Box<dyn Panel>,
            Box::new(panel_info::Panel::new()) as Box<dyn Panel>,
            Box::new(panel_error::Panel::new()) as Box<dyn Panel>,
        ];

        Self {
            panels,
            active_panel: 0,
        }
    }

    pub fn next_window(&mut self) {
        self.active_panel = (self.active_panel + 1) % self.panels.len();
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

        let [area_brief, area_right] = [layout[0], layout[1]];

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area_right);

        let [area_info, area_error] = [layout[0], layout[1]];

        for (index, window) in self.panels.iter().enumerate() {
            let block = Block::default()
                .title(window.title())
                .borders(Borders::ALL)
                .border_type(if index == self.active_panel {
                    BorderType::Double
                } else {
                    BorderType::Plain
                })
                .style(if index == self.active_panel {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                });

            let area_height = match window.title() {
                panel_brief::TITLE => area_brief.height,
                panel_info::TITLE => area_info.height,
                panel_error::TITLE => area_error.height,
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
                    panel_brief::TITLE => area_brief,
                    panel_info::TITLE => area_info,
                    panel_error::TITLE => area_error,
                    _ => panic!(),
                },
            );
        }

        let input = self.panels.get(self.active_panel).unwrap().input();

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
                    ret = self.panels.get_mut(self.active_panel).unwrap().key(key);
                }
            }
        }

        ret
    }
}
