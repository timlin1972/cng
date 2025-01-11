use log::Level::Error;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};
use tokio::sync::mpsc::Sender;

use crate::msg::{log, Data, DevInfo, Msg};
use crate::panels::{panel_brief, panel_devices, panel_error, panel_log};

pub const NAME: &str = "panels";

#[derive(PartialEq)]
pub enum RetKey {
    RKLeave,
    RKContinue,
}

#[derive(Debug)]
pub struct Popup {
    pub show: bool,
    pub title: String,
    pub x: u16,
    pub y: u16,
    pub text: String,
}

pub trait Panel {
    fn title(&self) -> &str;
    fn input(&self) -> &str;
    fn output(&self) -> &Vec<String>;
    fn output_clear(&mut self);
    fn output_push(&mut self, output: String);
    fn key(&mut self, key: KeyEvent) -> RetKey;
    fn run(&mut self, _cmd: &str) -> RetKey {
        RetKey::RKContinue
    }
    fn popup(&self) -> Option<&Popup>;
}

pub struct Panels {
    panels: Vec<Box<dyn Panel>>,
    active_panel: usize,
    msg_tx: Sender<Msg>,
}

impl Panels {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let panels = vec![
            Box::new(panel_log::Panel::new()) as Box<dyn Panel>,
            Box::new(panel_brief::Panel::new()) as Box<dyn Panel>,
            Box::new(panel_devices::Panel::new()) as Box<dyn Panel>,
            Box::new(panel_error::Panel::new()) as Box<dyn Panel>,
        ];

        Self {
            panels,
            active_panel: 1, // panel_brief
            msg_tx,
        }
    }

    fn next_window(&mut self) {
        self.active_panel = (self.active_panel + 1) % self.panels.len();
    }

    pub fn draw(&self, frame: &mut Frame) {
        // layout allocation
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(90), Constraint::Length(1)])
            .split(frame.area());

        // area_command
        let [area_top, area_command] = [layout[0], layout[1]];

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area_top);

        // area_left, area_right
        let [area_left, area_right] = [layout[0], layout[1]];

        // area_log, area_brief
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area_left);
        let [area_log, area_brief] = [layout[0], layout[1]];

        // area_info, area_error
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
                panel_log::TITLE => area_log.height,
                panel_brief::TITLE => area_brief.height,
                panel_devices::TITLE => area_info.height,
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
                    panel_log::TITLE => area_log,
                    panel_brief::TITLE => area_brief,
                    panel_devices::TITLE => area_info,
                    panel_error::TITLE => area_error,
                    _ => panic!(),
                },
            );
        }

        // Popup
        let popup = self.panels.get(self.active_panel).unwrap().popup();
        if let Some(popup) = popup {
            if popup.show {
                let popup_area = centered_rect(popup.x, popup.y, frame.area());

                frame.render_widget(Clear, popup_area);

                let popup_block = Block::default()
                    .borders(Borders::ALL)
                    .title(popup.title.clone())
                    .padding(ratatui::widgets::Padding::new(1, 1, 1, 1))
                    .style(Style::default().bg(Color::Black).fg(Color::White));
                frame.render_widget(popup_block.clone(), popup_area);

                let text = Paragraph::new(Text::from(popup.text.clone()))
                    .style(Style::default().fg(Color::Yellow));
                frame.render_widget(text, popup_block.inner(popup_area));
            }
        }

        // Command
        let input = self.panels.get(self.active_panel).unwrap().input();

        let paragraph_command = Paragraph::new(format!("> {input}")).style(Style::default());
        frame.render_widget(paragraph_command, area_command);

        frame.set_cursor_position(Position::new(
            area_command.x + input.len() as u16 + 2,
            area_command.y,
        ));
    }

    pub async fn key(&mut self, key: KeyEvent) -> RetKey {
        let mut ret = RetKey::RKContinue;

        match key.code {
            KeyCode::Tab => {
                self.next_window();
            }
            _ => {
                ret = self.panels.get_mut(self.active_panel).unwrap().key(key);
            }
        }

        ret
    }

    fn get_panel_mut(&mut self, name: &str) -> &mut Box<dyn Panel> {
        self.panels
            .iter_mut()
            .find(|p| p.title() == name)
            .unwrap_or_else(|| panic!("Panel not found: {}", name))
    }

    fn devices(&mut self, devices: Vec<DevInfo>) {
        self.get_panel_mut(panel_devices::TITLE).output_clear();
        self.get_panel_mut(panel_devices::TITLE)
            .output_push(format!("{:<16} {:<6}", "Name", "Onboard"));
        for device in devices.iter() {
            self.get_panel_mut(panel_devices::TITLE)
                .output_push(format!(
                    "{:<16} {:<6}",
                    device.name,
                    if device.onboard { "On" } else { "Off" }
                ));
        }
    }

    fn log(&mut self, level: log::Level, msg: String) {
        match level {
            log::Level::Info => self.get_panel_mut(panel_brief::TITLE).output_push(msg),
            log::Level::Debug | log::Level::Trace => {
                self.get_panel_mut(panel_log::TITLE).output_push(msg)
            }

            log::Level::Error | log::Level::Warn => {
                self.get_panel_mut(panel_error::TITLE).output_push(msg)
            }
        }
    }

    pub async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::Log(log) => {
                self.log(log.level, log.msg.clone());
            }
            Data::Devices(devices) => {
                self.devices(devices.clone());
            }
            _ => {
                log(
                    &self.msg_tx,
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, size: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(size);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
