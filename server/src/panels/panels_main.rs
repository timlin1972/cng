use async_trait::async_trait;
use log::Level::{Error, Info};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{log, Data, Msg, Reply};
use crate::panels::{panel_brief, panel_error, panel_infos, panel_log};

pub const NAME: &str = "panels";

#[derive(Debug)]
pub struct Popup {
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub output: Vec<String>,
    pub cursor_x: Option<usize>,
    pub cursor_y: Option<usize>,
}

#[derive(Debug)]
pub struct PanelInfo {
    pub name: String,
    pub input: String,
    pub output: Vec<String>,
    pub popup: Vec<Popup>,
    pub active_popup_name: Option<String>,
    pub msg_tx: Sender<Msg>,
}

impl PanelInfo {
    pub fn new(name: &str, popup: Vec<Popup>, msg_tx: Sender<Msg>) -> Self {
        Self {
            name: name.to_owned(),
            input: "".to_owned(),
            output: vec![],
            popup,
            active_popup_name: None,
            msg_tx,
        }
    }
}

#[async_trait]
pub trait Panel {
    fn get_panel_info(&self) -> &PanelInfo;
    fn title(&self) -> String {
        self.get_panel_info().name.to_owned()
    }
    async fn init(&mut self);
    async fn msg(&mut self, msg: &Msg);
    async fn key(&mut self, key: KeyEvent) -> bool;
    async fn run(&mut self, _cmd: &str) -> bool {
        false
    }
}

pub struct Panels {
    panels: Vec<Box<dyn Panel>>,
    active_panel: usize,
    msg_tx: Sender<Msg>,
}

impl Panels {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let panels = vec![
            Box::new(panel_infos::Panel::new(msg_tx.clone())) as Box<dyn Panel>,
            Box::new(panel_brief::Panel::new(msg_tx.clone())) as Box<dyn Panel>,
            Box::new(panel_log::Panel::new(msg_tx.clone())) as Box<dyn Panel>,
            Box::new(panel_error::Panel::new(msg_tx.clone())) as Box<dyn Panel>,
        ];

        Self {
            panels,
            active_panel: 1, // panel_brief
            msg_tx,
        }
    }

    pub async fn init(&mut self) {
        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] init"),
        )
        .await;
        for panel in &mut self.panels {
            panel.init().await;
        }
    }

    fn next_window(&mut self) {
        self.active_panel = (self.active_panel + 1) % self.panels.len();
    }

    fn popup(&self) -> Option<&Popup> {
        let panel_info = self.panels.get(self.active_panel).unwrap().get_panel_info();

        match panel_info.active_popup_name {
            None => None,
            Some(ref name) => panel_info.popup.iter().find(|&p| &p.name == name),
        }
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
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area_top);

        // area_left, area_right
        let [area_left, area_right] = [layout[0], layout[1]];

        // area_info, area_brief
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(area_left);
        let [area_info, area_brief] = [layout[0], layout[1]];

        // area_log, area_error
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area_right);
        let [area_log, area_error] = [layout[0], layout[1]];

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

            let area_height = match window.get_panel_info().name.as_str() {
                panel_log::NAME => area_log.height,
                panel_brief::NAME => area_brief.height,
                panel_infos::NAME => area_info.height,
                panel_error::NAME => area_error.height,
                _ => panic!(),
            };

            let window_output_len = window.get_panel_info().output.len() as u16;
            let scroll_offset = if window_output_len > (area_height - 2) {
                window_output_len - (area_height - 2)
            } else {
                0
            };

            let paragraph = Paragraph::new(window.get_panel_info().output.join("\n"))
                .block(block)
                .scroll((scroll_offset, 0));
            frame.render_widget(
                paragraph,
                match window.get_panel_info().name.as_str() {
                    panel_log::NAME => area_log,
                    panel_brief::NAME => area_brief,
                    panel_infos::NAME => area_info,
                    panel_error::NAME => area_error,
                    _ => panic!(),
                },
            );
        }

        // Popup
        let popup = self.popup();
        if let Some(popup) = popup {
            let popup_area = centered_rect(popup.x, popup.y, frame.area());

            frame.render_widget(Clear, popup_area);

            let popup_block = Block::default()
                .borders(Borders::ALL)
                .title(popup.name.clone())
                .padding(ratatui::widgets::Padding::new(0, 0, 0, 0))
                .style(Style::default().bg(Color::Black).fg(Color::White));
            frame.render_widget(popup_block.clone(), popup_area);

            let area_height = popup_area.height;

            let scroll_offset = if popup.output.len() as u16 > (area_height - 1) {
                popup.output.len() as u16 - (area_height - 1)
            } else {
                0
            };
            let text = Paragraph::new(Text::from(popup.output.join("\n")))
                .style(Style::default().fg(Color::Yellow))
                .scroll((scroll_offset, 0));

            frame.render_widget(text, popup_block.inner(popup_area));

            if let Some(cursor_x) = popup.cursor_x {
                if let Some(cursor_y) = popup.cursor_y {
                    frame.set_cursor_position(Position::new(
                        popup_area.x + cursor_x as u16 + 1,
                        popup_area.y + cursor_y as u16 + 1,
                    ));
                }
            }
        }

        // Command
        let input = &self
            .panels
            .get(self.active_panel)
            .unwrap()
            .get_panel_info()
            .input;

        let paragraph_command = Paragraph::new(format!("> {input}")).style(Style::default());
        frame.render_widget(paragraph_command, area_command);

        // if popup is not shown or non-interactive panel, set cursor position to the end of the command line
        if self.popup().is_none() {
            frame.set_cursor_position(Position::new(
                area_command.x + input.len() as u16 + 2,
                area_command.y,
            ));
        }
    }

    pub async fn key(&mut self, key: KeyEvent) -> bool {
        let mut ret = false;

        match key.code {
            KeyCode::Tab => match self.popup().is_some() {
                true => {
                    ret = self
                        .panels
                        .get_mut(self.active_panel)
                        .unwrap()
                        .key(key)
                        .await;
                }
                false => {
                    self.next_window();
                }
            },
            _ => {
                ret = self
                    .panels
                    .get_mut(self.active_panel)
                    .unwrap()
                    .key(key)
                    .await;
            }
        }

        ret
    }

    fn get_panel_mut(&mut self, name: &str) -> &mut Box<dyn Panel> {
        self.panels
            .iter_mut()
            .find(|p| p.get_panel_info().name == name)
            .unwrap_or_else(|| panic!("Panel not found: {}", name))
    }

    pub async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::Log(log) => match log.level {
                log::Level::Info => self.get_panel_mut(panel_brief::NAME).msg(msg).await,
                log::Level::Debug | log::Level::Trace => {
                    self.get_panel_mut(panel_log::NAME).msg(msg).await;
                }

                log::Level::Error | log::Level::Warn => {
                    self.get_panel_mut(panel_error::NAME).msg(msg).await;
                }
            },
            Data::Devices(_devices) => {
                self.get_panel_mut(panel_infos::NAME).msg(msg).await;
            }
            Data::DeviceCountdown => {
                self.get_panel_mut(panel_infos::NAME).msg(msg).await;
            }
            Data::Weather(_weather) => {
                self.get_panel_mut(panel_infos::NAME).msg(msg).await;
            }
            Data::Worldtime(_worldtime) => {
                self.get_panel_mut(panel_infos::NAME).msg(msg).await;
            }
            Data::Stocks(_stocks) => {
                self.get_panel_mut(panel_infos::NAME).msg(msg).await;
            }
            _ => {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
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

// common utils for Panel

const MAX_OUTPUT: usize = 512;

pub fn output_push(panel_info_output: &mut Vec<String>, output: String) {
    panel_info_output.push(output);
    if panel_info_output.len() > MAX_OUTPUT {
        panel_info_output.remove(0);
    }
}
