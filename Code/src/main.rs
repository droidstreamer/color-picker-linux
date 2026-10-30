// Native GNOME Colour Picker — GTK4 + libadwaita (gtk4-rs). Rust port of the C++ version.
// Picks screen colours via the XDG Desktop Portal.

mod colour;

use adw::prelude::*;
use colour::*;
use gtk::{cairo, gdk, gio, glib, pango};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::{Rc, Weak};

const APP_ID: &str = "io.github.droidstreamer.ColourPicker"; // also the icon name (see resources.xml)
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
// Set by the Makefile; "+" suffix = uncommitted changes.
const GIT_HASH: Option<&str> = option_env!("GIT_HASH");
const GIT_DATE: Option<&str> = option_env!("GIT_DATE");

// About > What's New. Newest first; AppStream markup (<p>, <ul>, <li>, <em>).
const RELEASE_NOTES: &str = concat!(
    "<p><em>1.0.0</em></p><ul>",
    "<li>First public release.</li>",
    "<li>Pick any pixel on screen and see its nearest name from the xkcd colour survey and the CSS colours.</li>",
    "<li>Fine-tune with HSV sliders and a shade bar; edit or copy HEX, RGB, HSL and CMYK.</li>",
    "<li>The last 14 colours stay in History.</li>",
    "</ul>",
);

// ---------------------------------------------------------------------------
// Screen colour picking via the XDG Desktop Portal
//   org.freedesktop.portal.Screenshot.PickColor
// ---------------------------------------------------------------------------
fn pick_colour(on_done: impl FnOnce(Result<Rgb, &'static str>) + 'static) {
    let bus = match gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("Failed to connect to session bus: {e}");
            return on_done(Err("Screen picking is unavailable."));
        }
    };
    let token = format!("clr{}", glib::random_int());
    let unique = bus.unique_name().map(|n| n[1..].replace('.', "_")).unwrap_or_default();
    let path = format!("/org/freedesktop/portal/desktop/request/{unique}/{token}");

    // Either the call fails or the Response signal arrives: whichever is first
    // unsubscribes and reports.
    let sub: Rc<RefCell<Option<gio::SignalSubscription>>> = Rc::default();
    let on_done = RefCell::new(Some(on_done));
    let finish: Rc<dyn Fn(Result<Rgb, &'static str>)> = Rc::new({
        let sub = sub.clone();
        move |result| {
            sub.borrow_mut().take();
            if let Some(f) = on_done.borrow_mut().take() {
                f(result)
            }
        }
    });
    let f = finish.clone();
    *sub.borrow_mut() = Some(bus.subscribe_to_signal(
        Some("org.freedesktop.portal.Desktop"),
        Some("org.freedesktop.portal.Request"),
        Some("Response"),
        Some(&path),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| f(portal_response(signal.parameters)),
    ));

    let opts = HashMap::from([("handle_token".to_string(), token.to_variant())]);
    bus.call(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
        "PickColor",
        Some(&("", opts).to_variant()),
        Some(glib::VariantTy::new("(o)").unwrap()),
        gio::DBusCallFlags::NONE,
        -1,
        gio::Cancellable::NONE,
        move |result| {
            if let Err(e) = result {
                eprintln!("PickColor failed: {e}");
                finish(Err("Screen picking is unavailable."));
            }
        },
    );
}

// The Response signal's (ua{sv}): code 0 = success, "color" = (ddd) in 0..1.
fn portal_response(params: &glib::Variant) -> Result<Rgb, &'static str> {
    if params.child_value(0).get::<u32>() != Some(0) {
        return Err("Picking was cancelled.");
    }
    let results = glib::VariantDict::new(Some(&params.child_value(1)));
    let (r, g, b) = results
        .lookup::<(f64, f64, f64)>("color")
        .ok()
        .flatten()
        .ok_or("No colour was returned.")?;
    let ch = |v: f64| ((v * 255.0).round() as i32).clamp(0, 255);
    Ok(rgb(ch(r), ch(g), ch(b)))
}

// ---------------------------------------------------------------------------
// Cairo helpers and a rounded, filled swatch.
// ---------------------------------------------------------------------------
fn rounded_path(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, rad: f64) {
    cr.new_sub_path();
    cr.arc(x + w - rad, y + rad, rad, -PI / 2.0, 0.0);
    cr.arc(x + w - rad, y + h - rad, rad, 0.0, PI / 2.0);
    cr.arc(x + rad, y + h - rad, rad, PI / 2.0, PI);
    cr.arc(x + rad, y + rad, rad, PI, 1.5 * PI);
    cr.close_path();
}

fn set_source(cr: &cairo::Context, c: Rgb) {
    cr.set_source_rgb(c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
}

fn swatch(colour: Rc<Cell<Rgb>>, radius: f64) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        rounded_path(cr, 0.0, 0.0, w, h, radius.min(w / 2.0).min(h / 2.0));
        set_source(cr, colour.get());
        let _ = cr.fill_preserve();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.12);
        cr.set_line_width(1.0);
        let _ = cr.stroke();
    });
    area
}

// A flat button holding a small rounded swatch; calls on_click(c) when pressed.
fn swatch_button(c: Rgb, size: i32, on_click: impl Fn(Rgb) + 'static) -> gtk::Button {
    let sw = swatch(Rc::new(Cell::new(c)), 8.0);
    sw.set_size_request(size, size);
    let btn = gtk::Button::builder().child(&sw).tooltip_text(hex(c)).build();
    btn.add_css_class("flat");
    btn.add_css_class("swatch-btn");
    btn.connect_clicked(move |_| on_click(c));
    btn
}

// ---------------------------------------------------------------------------
// A horizontal slider with a live gradient track and a round handle.
//   channel 'h' -> rainbow 0..360 ; 's' -> grey..saturated ; 'v' -> black..bright
// ---------------------------------------------------------------------------
const PAD: f64 = 12.0;
const TRACK_H: f64 = 12.0;
const HANDLE_R: f64 = 11.0;

struct Slider {
    area: gtk::DrawingArea,
    value: Rc<Cell<i32>>,
    label: gtk::Label,
}

impl Slider {
    fn set(&self, v: i32) {
        self.value.set(v);
        self.area.queue_draw();
        self.label.set_text(&v.to_string());
    }
}

fn colour_at(channel: char, hsv: Hsv, t: f64) -> Rgb {
    match channel {
        'h' => hsv_to_rgb(t * 360.0, 100.0, 100.0),
        's' => hsv_to_rgb(hsv.h as f64, t * 100.0, hsv.v as f64),
        _ => hsv_to_rgb(hsv.h as f64, hsv.s as f64, t * 100.0),
    }
}

// A row: letter, gradient slider, value. Appended to `col`.
fn grad_row(col: &gtk::Box, channel: char, maxv: i32, hsv: Rc<Cell<Hsv>>, me: Weak<Ui>) -> Slider {
    let tag = gtk::Label::new(Some(&channel.to_ascii_uppercase().to_string()));
    tag.add_css_class("dim-label");
    tag.add_css_class("numeric");
    tag.set_size_request(14, -1);
    tag.set_xalign(0.5);

    let area = gtk::DrawingArea::new();
    area.set_size_request(180, 30);
    area.set_hexpand(true);
    let value = Rc::new(Cell::new(0));
    area.set_draw_func({
        let value = value.clone();
        move |_, cr, w, h| {
            let hsv = hsv.get();
            let track_w = (w as f64 - 2.0 * PAD).max(1.0);
            let track_y = (h as f64 - TRACK_H) / 2.0;
            cr.save().ok();
            rounded_path(cr, PAD, track_y, track_w, TRACK_H, TRACK_H / 2.0);
            cr.clip();
            let steps = track_w as i32;
            for i in 0..steps {
                let t = i as f64 / (steps - 1).max(1) as f64;
                set_source(cr, colour_at(channel, hsv, t));
                cr.rectangle(PAD + i as f64, track_y, 2.0, TRACK_H);
                let _ = cr.fill();
            }
            cr.restore().ok();

            rounded_path(cr, PAD, track_y, track_w, TRACK_H, TRACK_H / 2.0);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.15);
            cr.set_line_width(1.0);
            let _ = cr.stroke();

            let t = value.get() as f64 / maxv as f64;
            let (hx, hy) = (PAD + t * track_w, h as f64 / 2.0);
            cr.arc(hx, hy, HANDLE_R, 0.0, 2.0 * PI);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.28);
            let _ = cr.stroke();
            cr.arc(hx, hy, HANDLE_R - 4.0, 0.0, 2.0 * PI);
            set_source(cr, colour_at(channel, hsv, t));
            let _ = cr.fill();
        }
    });

    let set_from_x = Rc::new({
        let (area, value) = (area.clone(), value.clone());
        move |x: f64| {
            let track_w = (area.width() as f64 - 2.0 * PAD).max(1.0);
            let v = (((x - PAD) / track_w).clamp(0.0, 1.0) * maxv as f64).round() as i32;
            if v != value.get() {
                value.set(v);
                area.queue_draw();
                if let Some(ui) = me.upgrade() {
                    ui.on_hsv_change(channel, v);
                }
            }
        }
    });
    let drag = gtk::GestureDrag::new();
    let start_x = Rc::new(Cell::new(0.0));
    drag.connect_drag_begin({
        let (start_x, set_from_x) = (start_x.clone(), set_from_x.clone());
        move |_, x, _| {
            start_x.set(x);
            set_from_x(x);
        }
    });
    drag.connect_drag_update(move |_, dx, _| set_from_x(start_x.get() + dx));
    area.add_controller(drag);

    let label = gtk::Label::new(Some("0"));
    label.add_css_class("numeric");
    label.add_css_class("dim-label");
    label.set_size_request(34, -1);
    label.set_xalign(1.0);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&tag);
    row.append(&area);
    row.append(&label);
    col.append(&row);
    Slider { area, value, label }
}

// ---------------------------------------------------------------------------
// Main window
// ---------------------------------------------------------------------------
type Parser = fn(&str) -> Option<Rgb>;
const FIELDS: [(&str, Parser); 4] =
    [("HEX", parse_hex), ("RGB", parse_rgb), ("HSL", parse_hsl), ("CMYK", parse_cmyk)];

struct Ui {
    me: Weak<Ui>, // for callbacks; signal handlers hold only Weak refs
    model: ColourModel,
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    current: Cell<Rgb>,
    hsv: Rc<Cell<Hsv>>,
    history: RefCell<Vec<Rgb>>,
    updating: Cell<bool>,
    editing: Cell<Option<usize>>,
    edit_start: Cell<Option<Rgb>>,
    edit_start_text: RefCell<String>,

    name_colour: Rc<Cell<Rgb>>,
    name_swatch: gtk::DrawingArea,
    name_label: gtk::Label,
    sliders: [Slider; 3],
    shade_box: gtk::Box,
    rows: [adw::EntryRow; 4],
    history_group: adw::PreferencesGroup,
    history_flow: gtk::FlowBox,
}

impl Ui {
    fn new(app: &adw::Application) -> Rc<Ui> {
        let ui = Rc::new_cyclic(|me: &Weak<Ui>| {
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Colour Picker")
                .default_width(560)
                .default_height(300)
                .resizable(false)
                .build();
            let toasts = adw::ToastOverlay::new();
            window.set_content(Some(&toasts));
            let toolbar = adw::ToolbarView::new();
            toasts.set_child(Some(&toolbar));
            let header = adw::HeaderBar::new();
            toolbar.add_top_bar(&header);

            let pick_btn = gtk::Button::from_icon_name("color-select-symbolic");
            pick_btn.set_tooltip_text(Some("Pick a Colour"));
            pick_btn.connect_clicked({
                let me = me.clone();
                move |_| {
                    let me = me.clone();
                    pick_colour(move |result| {
                        let Some(ui) = me.upgrade() else { return };
                        match result {
                            Ok(c) => ui.set_colour(c, true, None, None),
                            Err(msg) => ui.toasts.add_toast(adw::Toast::new(msg)),
                        }
                    });
                }
            });
            header.pack_start(&pick_btn);

            let menu = gio::Menu::new();
            menu.append(Some("About Colour Picker"), Some("win.about"));
            let menu_btn = gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).build();
            header.pack_end(&menu_btn);
            let about = gio::SimpleAction::new("about", None);
            about.connect_activate({
                let me = me.clone();
                move |_, _| {
                    if let Some(ui) = me.upgrade() {
                        ui.show_about();
                    }
                }
            });
            window.add_action(&about);

            let clamp = adw::Clamp::builder().maximum_size(560).tightening_threshold(520).build();
            toolbar.set_content(Some(&clamp));
            let outer = gtk::Box::new(gtk::Orientation::Vertical, 16);
            outer.set_margin_top(16);
            outer.set_margin_bottom(20);
            outer.set_margin_start(16);
            outer.set_margin_end(16);
            clamp.set_child(Some(&outer));

            // --- Cluster: name-swatch (left) + HSV sliders (right) ---
            let cluster = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            let name_colour = Rc::new(Cell::new(rgb(0, 0, 0)));
            let name_swatch = swatch(name_colour.clone(), 16.0);
            name_swatch.set_size_request(196, 132);
            name_swatch.set_valign(gtk::Align::Center);
            let name_overlay = gtk::Overlay::new();
            name_overlay.set_child(Some(&name_swatch));
            let name_label = gtk::Label::builder()
                .wrap(true)
                .wrap_mode(pango::WrapMode::WordChar)
                .justify(gtk::Justification::Center)
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .max_width_chars(13)
                .margin_start(8)
                .margin_end(8)
                .build();
            name_label.add_css_class("title-2");
            name_overlay.add_overlay(&name_label);
            cluster.append(&name_overlay);

            let sliders_col = gtk::Box::new(gtk::Orientation::Vertical, 10);
            sliders_col.set_hexpand(true);
            sliders_col.set_valign(gtk::Align::Center);
            let hsv = Rc::new(Cell::new(Hsv { h: 0, s: 0, v: 0 }));
            let sliders = [('h', 360), ('s', 100), ('v', 100)]
                .map(|(ch, maxv)| grad_row(&sliders_col, ch, maxv, hsv.clone(), me.clone()));
            cluster.append(&sliders_col);
            outer.append(&cluster);

            // --- Shade bar ---
            let shade_wrap = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let shade_labels = gtk::CenterBox::new();
            let caption = |text| {
                let l = gtk::Label::new(Some(text));
                l.add_css_class("dim-label");
                l.add_css_class("caption");
                l
            };
            shade_labels.set_start_widget(Some(&caption("Lighter")));
            shade_labels.set_center_widget(Some(&caption("Current Colour")));
            shade_labels.set_end_widget(Some(&caption("Darker")));
            shade_wrap.append(&shade_labels);
            let shade_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            shade_box.set_homogeneous(true);
            shade_wrap.append(&shade_box);
            outer.append(&shade_wrap);

            // --- Values (editable HEX / RGB / HSL / CMYK) ---
            let vgroup = adw::PreferencesGroup::new();
            let rows: [adw::EntryRow; 4] = std::array::from_fn(|i| entry_row(i, me));
            for row in &rows {
                vgroup.add(row);
            }
            outer.append(&vgroup);

            // --- History ---
            let history_group = adw::PreferencesGroup::builder().title("History").build();
            let history_flow = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .max_children_per_line(7)
                .min_children_per_line(7)
                .row_spacing(6)
                .column_spacing(6)
                .margin_top(4)
                .build();
            history_group.add(&history_flow);
            outer.append(&history_group);

            Ui {
                me: me.clone(),
                model: ColourModel::new(),
                window,
                toasts,
                current: Cell::new(rgb(52, 152, 219)),
                hsv,
                history: RefCell::default(),
                updating: Cell::new(false),
                editing: Cell::new(None),
                edit_start: Cell::new(None),
                edit_start_text: RefCell::default(),
                name_colour,
                name_swatch,
                name_label,
                sliders,
                shade_box,
                rows,
                history_group,
                history_flow,
            }
        });
        install_css();
        ui.set_colour(ui.current.get(), false, None, None);
        ui
    }

    // ---- editable value rows ----
    fn on_entry_changed(&self, i: usize) {
        if self.updating.get() || self.editing.get() != Some(i) {
            return;
        }
        let row = &self.rows[i];
        match FIELDS[i].1(&row.text()) {
            Some(c) => {
                row.remove_css_class("error");
                self.set_colour(c, false, None, Some(i));
            }
            None => row.add_css_class("error"),
        }
    }

    fn on_entry_focus_enter(&self, i: usize) {
        self.editing.set(Some(i));
        self.edit_start.set(Some(self.current.get()));
        *self.edit_start_text.borrow_mut() = self.rows[i].text().into();
    }

    fn on_entry_focus_leave(&self, i: usize) {
        self.editing.set(None);
        let row = &self.rows[i];
        row.remove_css_class("error");
        let text = row.text();
        if text != *self.edit_start_text.borrow() {
            match FIELDS[i].1(&text) {
                Some(c) => self.set_colour(c, Some(c) != self.edit_start.get(), None, None),
                None => self.refresh_entries(self.current.get(), None),
            }
        }
        self.edit_start.set(None);
    }

    fn on_entry_activated(&self, i: usize) {
        let row = &self.rows[i];
        let text = row.text();
        if text == *self.edit_start_text.borrow() {
            return;
        }
        match FIELDS[i].1(&text) {
            Some(c) => {
                let start = self.edit_start.get().unwrap_or(self.current.get());
                self.set_colour(c, c != start, None, None);
            }
            None => row.add_css_class("error"),
        }
    }

    fn on_hsv_change(&self, channel: char, v: i32) {
        if self.updating.get() {
            return;
        }
        let mut hsv = self.hsv.get();
        match channel {
            'h' => hsv.h = v,
            's' => hsv.s = v,
            _ => hsv.v = v,
        }
        self.set_colour(hsv_to_rgb(hsv.h as f64, hsv.s as f64, hsv.v as f64), false, Some(hsv), None);
    }

    // Same setup as ImageFlow: Credits/Legal/Website stay unset so the dialog
    // omits them; comments become the "Details" row, release notes "What's New".
    fn show_about(&self) {
        let version = match (GIT_HASH, GIT_DATE) {
            (Some(hash), Some(date)) if !hash.is_empty() && !date.is_empty() => format!("{APP_VERSION} ({date}, {hash})"),
            (Some(hash), _) if !hash.is_empty() => format!("{APP_VERSION} ({hash})"),
            _ => APP_VERSION.to_string(),
        };
        adw::AboutDialog::builder()
            .application_name("Colour Picker")
            .application_icon(APP_ID)
            .developer_name("Droid")
            .version(version)
            .comments(
                "Pick any colour on screen, see its nearest name, and fine-tune it with \
                 HSV sliders, shades and editable HEX / RGB / HSL / CMYK values.",
            )
            .release_notes_version(APP_VERSION)
            .release_notes(RELEASE_NOTES)
            .build()
            .present(Some(&self.window));
    }

    // ---- state ----
    fn set_colour(&self, c: Rgb, add_history: bool, hsv_override: Option<Hsv>, source_row: Option<usize>) {
        self.current.set(c);
        self.hsv.set(hsv_override.unwrap_or_else(|| rgb_to_hsv(c)));

        self.name_colour.set(c);
        self.name_swatch.queue_draw();
        self.name_label.set_text(&title_case(self.model.nearest_name(c)));
        let fg = if luminance(c) > 0.32 { 0 } else { 65535 };
        let attrs = pango::AttrList::new();
        attrs.insert(pango::AttrColor::new_foreground(fg, fg, fg));
        self.name_label.set_attributes(Some(&attrs));

        self.refresh_entries(c, source_row);
        let hsv = self.hsv.get();
        for (slider, v) in self.sliders.iter().zip([hsv.h, hsv.s, hsv.v]) {
            slider.set(v);
        }
        self.rebuild_shades(c);
        if add_history {
            self.push_history(c);
        }
    }

    fn refresh_entries(&self, c: Rgb, skip: Option<usize>) {
        let hsl = rgb_to_hsl(c);
        let k = rgb_to_cmyk(c);
        let texts = [
            hex(c),
            format!("rgb({}, {}, {})", c.r, c.g, c.b),
            format!("hsl({}, {}%, {}%)", hsl.h, hsl.s, hsl.l),
            format!("cmyk({}%, {}%, {}%, {}%)", k.c, k.m, k.y, k.k),
        ];
        self.updating.set(true);
        for (i, (row, text)) in self.rows.iter().zip(texts).enumerate() {
            if skip != Some(i) {
                row.set_text(&text);
            }
        }
        self.updating.set(false);
    }

    fn rebuild_shades(&self, c: Rgb) {
        while let Some(child) = self.shade_box.first_child() {
            self.shade_box.remove(&child);
        }
        for shade in perceptual_shade_row(c) {
            self.shade_box.append(&swatch_button(shade, 30, self.on_swatch_click()));
        }
    }

    // Defer to idle: clicking rebuilds the row that owns this very button, so
    // the rebuild must happen after this click signal finishes.
    fn on_swatch_click(&self) -> impl Fn(Rgb) + 'static {
        let me = self.me.clone();
        move |c| {
            let me = me.clone();
            glib::idle_add_local_once(move || {
                if let Some(ui) = me.upgrade() {
                    ui.set_colour(c, true, None, None);
                }
            });
        }
    }

    fn push_history(&self, c: Rgb) {
        let mut history = self.history.borrow_mut();
        history.retain(|&h| h != c);
        history.insert(0, c);
        history.truncate(14);
        self.history_flow.remove_all();
        for &h in history.iter() {
            self.history_flow.insert(&swatch_button(h, 34, self.on_swatch_click()), -1);
        }
        self.history_group.set_visible(!history.is_empty());
    }

    // ---- clipboard ----
    fn copy_value(&self, i: usize) {
        let text = self.rows[i].text();
        self.window.clipboard().set_text(&text);
        let toast = adw::Toast::new(&format!("Copied {}  {}", FIELDS[i].0, text));
        toast.set_timeout(2);
        self.toasts.add_toast(toast);
    }

}

fn entry_row(i: usize, me: &Weak<Ui>) -> adw::EntryRow {
    let row = adw::EntryRow::builder().title(FIELDS[i].0).build();
    let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
    copy.add_css_class("flat");
    copy.set_valign(gtk::Align::Center);
    copy.set_tooltip_text(Some("Copy"));
    let on = |f: fn(&Ui, usize)| {
        let me = me.clone();
        move || {
            if let Some(ui) = me.upgrade() {
                f(&ui, i)
            }
        }
    };
    let copy_cb = on(Ui::copy_value);
    copy.connect_clicked(move |_| copy_cb());
    row.add_suffix(&copy);

    let changed = on(Ui::on_entry_changed);
    row.connect_changed(move |_| changed());
    let activated = on(Ui::on_entry_activated);
    row.connect_entry_activated(move |_| activated());
    let focus = gtk::EventControllerFocus::new();
    let enter = on(Ui::on_entry_focus_enter);
    focus.connect_enter(move |_| enter());
    let leave = on(Ui::on_entry_focus_leave);
    focus.connect_leave(move |_| leave());
    row.add_controller(focus);
    row
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(".swatch-btn { padding: 0; min-width: 0; min-height: 0; }\n");
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

// Screenshot harness (like Diskopium's): COLOURPICKER_SNAPSHOT=out.png renders the
// window at 2x to a PNG and quits. COLOURPICKER_HISTORY="#hex,#hex" fills History
// (oldest first), COLOURPICKER_COLOUR="#hex" sets the current colour.
fn snapshot_mode(ui: &Ui) {
    let Ok(out) = std::env::var("COLOURPICKER_SNAPSHOT") else { return };
    let var = |k| std::env::var(k).unwrap_or_default();
    for c in var("COLOURPICKER_HISTORY").split(',').filter_map(|s| parse_hex(s.trim())) {
        ui.set_colour(c, true, None, None);
    }
    if let Some(c) = parse_hex(&var("COLOURPICKER_COLOUR")) {
        ui.set_colour(c, false, None, None);
    }
    let win = ui.window.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
        let (w, h) = (win.width() as f32, win.height() as f32);
        let snap = gtk::Snapshot::new();
        snap.scale(2.0, 2.0);
        gtk::WidgetPaintable::new(Some(&win)).snapshot(&snap, w as f64, h as f64);
        if let (Some(node), Some(r)) = (snap.to_node(), win.native().and_then(|n| n.renderer())) {
            let _ = r.render_texture(&node, None).save_to_png(&out);
        }
        win.destroy();
    });
}

fn main() -> glib::ExitCode {
    // The app icon, embedded (build.rs); GtkApplication finds it under its id.
    let res = gio::Resource::from_data(&glib::Bytes::from_static(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/colourpicker.gresource"
    ))))
    .expect("embedded resources");
    gio::resources_register(&res);
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        gtk::Window::set_default_icon_name(APP_ID);
        let ui = Ui::new(app);
        ui.window.present();
        snapshot_mode(&ui);
        // ponytail: the Ui lives as long as the app, like the C++ MainWindow did.
        std::mem::forget(ui);
    });
    app.run()
}
