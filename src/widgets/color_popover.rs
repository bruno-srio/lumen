use gdk::prelude::*;
use gettextrs::gettext;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug)]
pub enum TintChoice {
    Hsv { hue: u16, sat: u8 },
    Temperature(u32),
}

type ColorCallback = Box<dyn Fn(TintChoice)>;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/bruno_srio/Lumen/color_popover.ui")]
    pub struct ColorPopover {
        #[template_child]
        pub swatches: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub hue_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub sat_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub custom_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub color_section: TemplateChild<gtk::Box>,
        #[template_child]
        pub white_section: TemplateChild<gtk::Box>,
        #[template_child]
        pub temperature_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub saved_swatches: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        pub updating: Cell<bool>,
        pub callback: RefCell<Option<ColorCallback>>,
        pub save_callback: RefCell<Option<ColorCallback>>,
        pub remove_callback: RefCell<Option<Box<dyn Fn(usize)>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ColorPopover {
        const NAME: &'static str = "LumenColorPopover";
        type Type = super::ColorPopover;
        type ParentType = gtk::Popover;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ColorPopover {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.populate_swatches();
            obj.connect_scales();
            obj.connect_custom();
            obj.connect_temperature();
            obj.wire_save();
        }
    }
    impl WidgetImpl for ColorPopover {}
    impl PopoverImpl for ColorPopover {}
}

glib::wrapper! {
    pub struct ColorPopover(ObjectSubclass<imp::ColorPopover>)
        @extends gtk::Popover, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::ShortcutManager;
}

impl ColorPopover {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_selected(&self, callback: impl Fn(TintChoice) + 'static) {
        self.imp().callback.replace(Some(Box::new(callback)));
    }

    pub fn connect_save(&self, callback: impl Fn(TintChoice) + 'static) {
        self.imp().save_callback.replace(Some(Box::new(callback)));
    }

    pub fn connect_remove_saved(&self, callback: impl Fn(usize) + 'static) {
        self.imp().remove_callback.replace(Some(Box::new(callback)));
    }

    pub fn set_capabilities(
        &self,
        has_color: bool,
        has_temperature: bool,
        ct_min: f64,
        ct_max: f64,
    ) {
        self.imp().color_section.set_visible(has_color);
        self.imp().white_section.set_visible(has_temperature);
        self.imp().temperature_scale.set_range(ct_min, ct_max);
    }

    pub fn set_saved(&self, saved: Vec<(String, TintChoice)>) {
        let flow = self.imp().saved_swatches.get();
        while let Some(child) = flow.first_child() {
            flow.remove(&child);
        }
        for (index, (name, choice)) in saved.into_iter().enumerate() {
            let button = gtk::Button::new();
            button.add_css_class("flat");
            button.set_tooltip_text(Some(&name));
            let swatch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            swatch.set_size_request(28, 28);
            swatch.add_css_class("color-swatch");
            let (red, green, blue) = choice_rgb(choice);
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&format!(
                "box {{ background-color: rgb({red}, {green}, {blue}); border-radius: 14px; }}"
            ));
            #[allow(deprecated)]
            {
                swatch
                    .style_context()
                    .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            }
            button.set_child(Some(&swatch));
            let popover = self.downgrade();
            button.connect_clicked(move |_| {
                if let Some(popover) = popover.upgrade() {
                    popover.emit_choice(choice);
                }
            });
            let popover = self.downgrade();
            let gesture = gtk::GestureClick::new();
            gesture.set_button(3);
            gesture.connect_released(move |gesture, _, _, _| {
                if let Some(popover) = popover.upgrade() {
                    if let Some(callback) = popover.imp().remove_callback.borrow().as_ref() {
                        callback(index);
                    }
                }
                gesture.set_state(gtk::EventSequenceState::Claimed);
            });
            button.add_controller(gesture);
            let popover = self.downgrade();
            let long_press = gtk::GestureLongPress::new();
            long_press.connect_pressed(move |_, _, _| {
                if let Some(popover) = popover.upgrade() {
                    if let Some(callback) = popover.imp().remove_callback.borrow().as_ref() {
                        callback(index);
                    }
                }
            });
            button.add_controller(long_press);
            flow.append(&button);
        }
    }

    pub fn set_temperature(&self, kelvin: u32) {
        let imp = self.imp();
        imp.updating.set(true);
        imp.temperature_scale.set_value(f64::from(kelvin));
        imp.updating.set(false);
    }

    pub fn set_color(&self, hue: u16, sat: u8) {
        let imp = self.imp();
        imp.updating.set(true);
        imp.hue_scale.set_value(f64::from(hue % 360));
        imp.sat_scale.set_value(f64::from(sat.min(100)));
        imp.updating.set(false);
    }

    fn emit_selected(&self) {
        if self.imp().updating.get() {
            return;
        }
        let hue = self.imp().hue_scale.value().round() as u16;
        let sat = self.imp().sat_scale.value().round() as u8;
        self.emit_choice(TintChoice::Hsv { hue, sat });
    }

    fn emit_choice(&self, choice: TintChoice) {
        if let Some(callback) = self.imp().callback.borrow().as_ref() {
            callback(choice);
        }
    }

    fn current_choice(&self) -> TintChoice {
        if self.imp().white_section.is_visible() && !self.imp().color_section.is_visible() {
            TintChoice::Temperature(self.imp().temperature_scale.value().round() as u32)
        } else {
            TintChoice::Hsv {
                hue: self.imp().hue_scale.value().round() as u16,
                sat: self.imp().sat_scale.value().round() as u8,
            }
        }
    }

    fn connect_temperature(&self) {
        let popover = self.downgrade();
        self.imp()
            .temperature_scale
            .connect_change_value(move |scale, _, value| {
                let Some(popover) = popover.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                if popover.imp().updating.get() {
                    return glib::Propagation::Proceed;
                }
                let min = scale.adjustment().lower();
                let max = scale.adjustment().upper();
                let value = value.clamp(min, max);
                scale.set_value(value);
                popover.emit_choice(TintChoice::Temperature(value.round() as u32));
                glib::Propagation::Stop
            });
    }

    fn wire_save(&self) {
        let popover = self.downgrade();
        self.imp().save_button.connect_clicked(move |_| {
            let Some(popover) = popover.upgrade() else {
                return;
            };
            let choice = popover.current_choice();
            let imp = popover.imp();
            let borrowed = imp.save_callback.borrow();
            if let Some(callback) = borrowed.as_ref() {
                callback(choice);
            }
        });
    }

    fn populate_swatches(&self) {
        let flow = self.imp().swatches.clone();
        let presets = [
            (gettext("Red"), "swatch-red", 0, 100),
            (gettext("Orange"), "swatch-orange", 30, 100),
            (gettext("Yellow"), "swatch-yellow", 55, 100),
            (gettext("Green"), "swatch-green", 130, 100),
            (gettext("Cyan"), "swatch-cyan", 180, 100),
            (gettext("Blue"), "swatch-blue", 230, 100),
            (gettext("Purple"), "swatch-purple", 280, 100),
            (gettext("Pink"), "swatch-pink", 320, 70),
            (gettext("White"), "swatch-white", 0, 0),
        ];
        for (label, class_name, hue, sat) in presets {
            let button = gtk::Button::new();
            button.add_css_class("color-swatch");
            button.add_css_class(class_name);
            button.set_tooltip_text(Some(label.as_str()));
            button.set_accessible_role(gtk::AccessibleRole::Button);
            button.update_property(&[gtk::accessible::Property::Label(label.as_str())]);
            let popover = self.downgrade();
            let hue = hue as u16;
            let sat = sat as u8;
            button.connect_clicked(move |_| {
                let Some(popover) = popover.upgrade() else {
                    return;
                };
                popover.set_color(hue, sat);
                popover.emit_selected();
            });
            flow.append(&button);
        }
    }

    fn connect_scales(&self) {
        for scale in [&self.imp().hue_scale, &self.imp().sat_scale] {
            let popover = self.downgrade();
            scale.connect_change_value(move |scale, _, value| {
                scale.set_value(value);
                if let Some(popover) = popover.upgrade() {
                    popover.emit_selected();
                }
                glib::Propagation::Stop
            });
        }
    }

    fn connect_custom(&self) {
        let popover = self.downgrade();
        self.imp().custom_button.connect_clicked(move |_| {
            let Some(popover) = popover.upgrade() else {
                return;
            };
            let hue = popover.imp().hue_scale.value().round() as u16;
            let sat = popover.imp().sat_scale.value().round() as u8;
            let rgb = yeelight::hs_to_rgb(hue, sat);
            let initial = gdk::RGBA::new(
                f32::from(rgb.r) / 255.0,
                f32::from(rgb.g) / 255.0,
                f32::from(rgb.b) / 255.0,
                1.0,
            );
            let dialog = gtk::ColorDialog::builder()
                .with_alpha(false)
                .title(gettext("Custom Color"))
                .modal(true)
                .build();
            let parent = popover.root().and_downcast::<gtk::Window>();
            let popover = popover.clone();
            dialog.choose_rgba(
                parent.as_ref(),
                Some(&initial),
                None::<&gio::Cancellable>,
                move |result| {
                    let Ok(color) = result else {
                        return;
                    };
                    let rgb = yeelight::Rgb::new(
                        (color.red() * 255.0).round() as u8,
                        (color.green() * 255.0).round() as u8,
                        (color.blue() * 255.0).round() as u8,
                    );
                    let (hue, sat) = yeelight::rgb_to_hs(rgb);
                    popover.set_color(hue, sat);
                    popover.emit_selected();
                },
            );
        });
    }
}

fn choice_rgb(choice: TintChoice) -> (u8, u8, u8) {
    match choice {
        TintChoice::Hsv { hue, sat } => {
            let rgb = yeelight::hs_to_rgb(hue, sat);
            (rgb.r, rgb.g, rgb.b)
        }
        TintChoice::Temperature(kelvin) => {
            let rgb = yeelight::ct_to_rgb(kelvin);
            (rgb.r, rgb.g, rgb.b)
        }
    }
}

impl Default for ColorPopover {
    fn default() -> Self {
        Self::new()
    }
}
