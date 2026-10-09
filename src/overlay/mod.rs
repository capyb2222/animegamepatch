mod client;

use hudhook::hooks::dx11::ImguiDx11Hooks;
use hudhook::imgui::{
    Condition, Context, FontConfig, FontId, FontSource, StyleColor, StyleVar, Ui, WindowFlags,
};
use hudhook::windows::Win32::Foundation::HINSTANCE;
use hudhook::{Hudhook, ImguiRenderLoop, RenderContext};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

use std::sync::atomic::Ordering;

use crate::config::{self, FPS_ENABLED, FPS_TARGET, SHOW_PANEL};
use crate::modules::fps;
use client::{Row, Snapshot};

const VK_CONTROL: i32 = 0x11;
const VK_K: i32 = 0x4B;
const VK_L: i32 = 0x4C;

const FPS_PRESETS: [i32; 5] = [60, 120, 144, 165, 240];

const FONT_REGULAR: &str = "C:\\Windows\\Fonts\\segoeui.ttf";
const FONT_BOLD: &str = "C:\\Windows\\Fonts\\segoeuib.ttf";
const FONT_SIZE: f32 = 18.0;

const TEXT: [f32; 4] = [0.93, 0.90, 0.85, 1.0];
const MUTED: [f32; 4] = [0.66, 0.67, 0.72, 1.0];
const GOLD: [f32; 4] = [1.0, 0.84, 0.48, 1.0];
const TRACK: [f32; 4] = [1.0, 1.0, 1.0, 0.08];
const ROW_BG: [f32; 4] = [1.0, 1.0, 1.0, 0.04];

#[derive(Copy, Clone, PartialEq)]
enum Tab {
    Dealt,
    Taken,
    Log,
}

pub struct Overlay {
    menu: bool,
    menu_held: bool,
    panel_held: bool,
    fps_input: i32,
    tab: Tab,
    bold: bool,
}

fn element_color(element: u32) -> [f32; 4] {
    match element {
        1 => [1.0, 0.45, 0.30, 1.0],
        2 => [0.30, 0.68, 1.0, 1.0],
        3 => [0.55, 0.85, 0.25, 1.0],
        4 => [0.76, 0.52, 1.0, 1.0],
        5 | 6 => [0.62, 0.90, 0.98, 1.0],
        7 => [0.42, 0.92, 0.78, 1.0],
        8 => [1.0, 0.80, 0.32, 1.0],
        _ => [0.85, 0.85, 0.88, 1.0],
    }
}

fn group(value: f64) -> String {
    let digits = (value.max(0.0).round() as u64).to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn load_font(ctx: &mut Context, path: &str) -> Option<FontId> {
    let data: &'static [u8] = Box::leak(std::fs::read(path).ok()?.into_boxed_slice());
    Some(ctx.fonts().add_font(&[FontSource::TtfData {
        data,
        size_pixels: FONT_SIZE,
        config: Some(FontConfig {
            oversample_h: 2,
            ..FontConfig::default()
        }),
    }]))
}

impl Overlay {
    fn new() -> Self {
        Self {
            menu: false,
            menu_held: false,
            panel_held: false,
            fps_input: FPS_TARGET.load(Ordering::Relaxed),
            tab: Tab::Dealt,
            bold: false,
        }
    }

    fn poll_keys(&mut self) {
        let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL) < 0 };
        let menu = ctrl && unsafe { GetAsyncKeyState(VK_L) < 0 };
        let panel = ctrl && unsafe { GetAsyncKeyState(VK_K) < 0 };

        if menu && !self.menu_held {
            self.menu = !self.menu;
        }
        if panel && !self.panel_held {
            SHOW_PANEL.fetch_xor(true, Ordering::Relaxed);
            config::save();
        }

        self.menu_held = menu;
        self.panel_held = panel;
    }

    fn set_fps(&mut self, value: i32) {
        let value = value.clamp(0, 1000);
        self.fps_input = value;
        FPS_TARGET.store(value, Ordering::Relaxed);
        config::save();
        fps::apply();
    }

    fn settings(&mut self, ui: &Ui) {
        self.bold_text(ui, GOLD, "Damage statistics");
        let mut show = SHOW_PANEL.load(Ordering::Relaxed);
        if ui.checkbox("Show damage panel", &mut show) {
            SHOW_PANEL.store(show, Ordering::Relaxed);
            config::save();
        }
        ui.text_colored(MUTED, "Ctrl+K toggles the panel anywhere.");

        ui.spacing();
        ui.separator();
        ui.spacing();

        self.bold_text(ui, GOLD, "FPS unlocker");
        if !fps::available() {
            ui.text_colored(MUTED, "Not available on this game version.");
            return;
        }

        let mut enabled = FPS_ENABLED.load(Ordering::Relaxed);
        if ui.checkbox("Unlock frame rate", &mut enabled) {
            FPS_ENABLED.store(enabled, Ordering::Relaxed);
            config::save();
            fps::apply();
        }

        ui.set_next_item_width(140.0);
        let submitted = ui
            .input_int("Target FPS", &mut self.fps_input)
            .enter_returns_true(true)
            .build();
        if submitted {
            self.set_fps(self.fps_input);
        }

        for preset in FPS_PRESETS {
            if ui.button(preset.to_string()) {
                self.set_fps(preset);
            }
            ui.same_line();
        }
        if ui.button("Unlimited") {
            self.set_fps(0);
        }

        let current = match FPS_TARGET.load(Ordering::Relaxed) {
            0 => "unlimited".to_string(),
            value => value.to_string(),
        };
        ui.text_colored(
            MUTED,
            format!("Current target: {current}. Press Enter to apply a typed value."),
        );
    }

    fn panel(&mut self, ui: &Ui) {
        let snap = client::snapshot();
        let display = ui.io().display_size;
        let _min = ui.push_style_var(StyleVar::WindowMinSize([300.0, 90.0]));

        ui.window("##lunagc-damage")
            .position([display[0] - 400.0, 150.0], Condition::FirstUseEver)
            .size([370.0, 0.0], Condition::FirstUseEver)
            .flags(
                WindowFlags::NO_TITLE_BAR
                    | WindowFlags::NO_SCROLLBAR
                    | WindowFlags::NO_COLLAPSE
                    | WindowFlags::ALWAYS_AUTO_RESIZE
                    | WindowFlags::NO_FOCUS_ON_APPEARING
                    | WindowFlags::NO_NAV,
            )
            .build(|| {
                ui.dummy([342.0, 0.0]);
                self.header(ui, &snap);
                ui.separator();
                match self.tab {
                    Tab::Dealt => self.rows(ui, &snap, false),
                    Tab::Taken => self.rows(ui, &snap, true),
                    Tab::Log => self.log(ui, &snap),
                }
                self.footer(ui, &snap);
            });
    }

    fn bold_font(&self, ui: &Ui) -> Option<FontId> {
        self.bold.then(|| ui.fonts().fonts().get(1).copied()).flatten()
    }

    fn bold_text(&self, ui: &Ui, color: [f32; 4], text: &str) {
        let _font = self.bold_font(ui).map(|id| ui.push_font(id));
        ui.text_colored(color, text);
    }

    fn tab_button(&mut self, ui: &Ui, label: &str, tab: Tab) {
        let selected = self.tab == tab;
        let _text = ui.push_style_color(StyleColor::Text, if selected { GOLD } else { MUTED });
        let _bg = ui.push_style_color(
            StyleColor::Button,
            if selected { [1.0, 0.84, 0.48, 0.16] } else { [1.0, 1.0, 1.0, 0.05] },
        );
        if ui.button(label) {
            self.tab = tab;
        }
    }

    fn header(&mut self, ui: &Ui, snap: &Snapshot) {
        self.bold_text(ui, GOLD, "Damage Statistics");

        let status = if !snap.connected {
            "server offline"
        } else if !snap.online {
            "not in game"
        } else if snap.active {
            "in combat"
        } else {
            "idle"
        };
        let width = ui.calc_text_size(status)[0];
        ui.same_line_with_pos(ui.window_size()[0] - width - 14.0);
        ui.text_colored(if snap.active { GOLD } else { MUTED }, status);

        self.tab_button(ui, "Dealt", Tab::Dealt);
        ui.same_line();
        self.tab_button(ui, "Taken", Tab::Taken);
        ui.same_line();
        self.tab_button(ui, "Log", Tab::Log);

        let width = ui.calc_text_size("Reset")[0] + 20.0;
        ui.same_line_with_pos(ui.window_size()[0] - width - 14.0);
        let _text = ui.push_style_color(StyleColor::Text, MUTED);
        if ui.button_with_size("Reset", [width, 0.0]) {
            client::request_reset();
        }
    }

    fn rows(&self, ui: &Ui, snap: &Snapshot, taken: bool) {
        let value = |row: &Row| if taken { row.taken } else { row.dealt };
        let mut rows: Vec<&Row> = snap.rows.iter().filter(|r| value(r) > 0.0).collect();
        rows.sort_by(|a, b| value(b).total_cmp(&value(a)));

        if rows.is_empty() {
            ui.text_colored(MUTED, if taken { "No damage taken yet." } else { "No damage dealt yet." });
            return;
        }

        let total: f64 = rows.iter().map(|r| value(r)).sum();
        let top = value(rows[0]);
        let multiplayer = rows.iter().any(|r| r.owner != rows[0].owner);
        let line = ui.text_line_height();
        let draw = ui.get_window_draw_list();

        for (rank, row) in rows.iter().enumerate() {
            let color = element_color(row.element);
            let amount = value(row);
            let origin = ui.cursor_screen_pos();
            let width = ui.content_region_avail()[0];
            let detail = !taken;
            let height = line * if detail { 2.0 } else { 1.0 } + 16.0;

            draw.add_rect(origin, [origin[0] + width, origin[1] + height], ROW_BG)
                .filled(true)
                .rounding(6.0)
                .build();
            draw.add_rect(origin, [origin[0] + 4.0, origin[1] + height], color)
                .filled(true)
                .rounding(2.0)
                .build();

            let left = origin[0] + 12.0;
            let right = origin[0] + width - 8.0;
            let y = origin[1] + 4.0;

            let name = if multiplayer {
                format!("{}  {} ({})", rank + 1, row.name, row.owner)
            } else {
                format!("{}  {}", rank + 1, row.name)
            };
            {
                let _font = self.bold_font(ui).map(|id| ui.push_font(id));
                draw.add_text([left, y], TEXT, &name);
            }

            let share = if total > 0.0 { amount / total * 100.0 } else { 0.0 };
            let share_text = format!("{share:.1}%");
            let share_width = ui.calc_text_size(&share_text)[0];
            draw.add_text([right - share_width, y], MUTED, &share_text);

            let amount_text = group(amount);
            {
                let _font = self.bold_font(ui).map(|id| ui.push_font(id));
                let amount_width = ui.calc_text_size(&amount_text)[0];
                draw.add_text([right - 58.0 - amount_width, y], color, &amount_text);
            }

            let mut bar_y = y + line + 3.0;
            if detail {
                let stats = format!(
                    "{}/s    {} hits    max {}",
                    group(row.dps),
                    row.hits,
                    group(row.max_hit)
                );
                draw.add_text([left, y + line], MUTED, &stats);
                bar_y += line;
            }

            let fill = if top > 0.0 { (amount / top) as f32 } else { 0.0 };
            draw.add_rect([left, bar_y], [right, bar_y + 4.0], TRACK)
                .filled(true)
                .rounding(2.0)
                .build();
            draw.add_rect(
                [left, bar_y],
                [left + (right - left) * fill.clamp(0.02, 1.0), bar_y + 4.0],
                color,
            )
            .filled(true)
            .rounding(2.0)
            .build();

            ui.dummy([width, height]);
        }
    }

    fn log(&self, ui: &Ui, snap: &Snapshot) {
        if snap.hits.is_empty() {
            ui.text_colored(MUTED, "No hits yet.");
            return;
        }

        let right = ui.window_size()[0] - 14.0;
        for hit in snap.hits.iter().rev().take(14) {
            ui.text_colored(MUTED, format!("{:>4.0}s", hit.ago));
            ui.same_line();
            ui.text_colored(TEXT, &hit.name);
            ui.same_line();
            ui.text_colored(MUTED, format!("> {}", hit.target));

            let amount = group(hit.damage);
            let width = ui.calc_text_size(&amount)[0];
            ui.same_line_with_pos(right - width);
            ui.text_colored(element_color(hit.element), amount);
        }
    }

    fn footer(&self, ui: &Ui, snap: &Snapshot) {
        ui.separator();
        ui.text_colored(MUTED, "Total");
        ui.same_line();
        let total = if self.tab == Tab::Taken { snap.total_taken } else { snap.total };
        self.bold_text(ui, TEXT, &group(total));

        let tail = format!("{}/s   {}", group(snap.dps), clock(snap.duration));
        let width = ui.calc_text_size(&tail)[0];
        ui.same_line_with_pos(ui.window_size()[0] - width - 14.0);
        ui.text_colored(MUTED, tail);
    }
}

impl ImguiRenderLoop for Overlay {
    fn initialize<'a>(&'a mut self, ctx: &mut Context, _: &'a mut dyn RenderContext) {
        ctx.set_ini_filename(None);

        if load_font(ctx, FONT_REGULAR).is_some() {
            self.bold = load_font(ctx, FONT_BOLD).is_some();
        }

        let style = ctx.style_mut();
        style.window_rounding = 10.0;
        style.window_border_size = 1.0;
        style.window_padding = [14.0, 12.0];
        style.frame_rounding = 6.0;
        style.frame_padding = [10.0, 3.0];
        style.item_spacing = [6.0, 6.0];
        style.colors[StyleColor::WindowBg as usize] = [0.07, 0.08, 0.11, 0.82];
        style.colors[StyleColor::Border as usize] = [1.0, 0.84, 0.48, 0.25];
        style.colors[StyleColor::Separator as usize] = [1.0, 1.0, 1.0, 0.10];
        style.colors[StyleColor::Text as usize] = TEXT;
        style.colors[StyleColor::Button as usize] = [1.0, 1.0, 1.0, 0.05];
        style.colors[StyleColor::ButtonHovered as usize] = [1.0, 0.84, 0.48, 0.22];
        style.colors[StyleColor::ButtonActive as usize] = [1.0, 0.84, 0.48, 0.32];
        style.colors[StyleColor::ResizeGrip as usize] = [0.0, 0.0, 0.0, 0.0];
        style.colors[StyleColor::TitleBg as usize] = [0.07, 0.08, 0.11, 0.95];
        style.colors[StyleColor::TitleBgActive as usize] = [0.12, 0.13, 0.17, 0.95];
        style.colors[StyleColor::FrameBg as usize] = [1.0, 1.0, 1.0, 0.07];
        style.colors[StyleColor::FrameBgHovered as usize] = [1.0, 0.84, 0.48, 0.18];
        style.colors[StyleColor::FrameBgActive as usize] = [1.0, 0.84, 0.48, 0.28];
        style.colors[StyleColor::CheckMark as usize] = GOLD;
    }

    fn render(&mut self, ui: &mut Ui) {
        self.poll_keys();

        if SHOW_PANEL.load(Ordering::Relaxed) {
            self.panel(ui);
        }

        if self.menu {
            let mut open = true;
            ui.window("LunaGC settings")
                .position([80.0, 150.0], Condition::FirstUseEver)
                .flags(
                    WindowFlags::ALWAYS_AUTO_RESIZE
                        | WindowFlags::NO_COLLAPSE
                        | WindowFlags::NO_SAVED_SETTINGS,
                )
                .opened(&mut open)
                .build(|| self.settings(ui));
            self.menu = open;
        }
    }
}

pub fn start(module: usize) {
    client::start();

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));

        let result = Hudhook::builder()
            .with::<ImguiDx11Hooks>(Overlay::new())
            .with_hmodule(HINSTANCE(module as _))
            .build()
            .apply();

        match result {
            Ok(()) => crate::plog!("Overlay ready: Ctrl+L opens settings, Ctrl+K toggles the damage panel"),
            Err(e) => crate::plog!("Damage overlay failed to hook: {e:?}"),
        }
    });
}
