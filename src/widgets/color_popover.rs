use gdk::prelude::*;
use glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

type ColorCallback = Box<dyn Fn(u16, u8)>;

const PRESETS: &[(&str, &str, u16, u8)] = &[
    ("Red", "swatch-red", 0, 100),
    ("Orange", "swatch-orange", 30, 100),
    ("Yellow", "swatch-yellow", 55, 100),
    ("Green", "swatch-green", 130, 100),
    ("Cyan", "swatch-cyan", 180, 100),
    ("Blue", "swatch-blue", 230, 100),
    ("Purple", "swatch-purple", 280, 100),
    ("Pink", "swatch-pink", 320, 70),
    ("White", "swatch-white", 0, 0),
];

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
        pub updating: Cell<bool>,
        pub callback: RefCell<Option<ColorCallback>>,
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

    pub fn connect_selected(&self, callback: impl Fn(u16, u8) + 'static) {
        self.imp().callback.replace(Some(Box::new(callback)));
    }

    pub fn set_color(&self, hue: u16, sat: u8) {
        let imp = self.imp();
        imp.updating.set(true);
        imp.hue_scale.set_value(f64::from(hue % 360));
        imp.sat_scale.set_value(f64::from(sat.min(100)));
        imp.updating.set(false);
    }

    fn emit_selected(&self) {
        let imp = self.imp();
        if imp.updating.get() {
            return;
        }
        let hue = imp.hue_scale.value().round() as u16;
        let sat = imp.sat_scale.value().round() as u8;
        if let Some(callback) = imp.callback.borrow().as_ref() {
            callback(hue, sat);
        }
    }

    fn populate_swatches(&self) {
        let flow = self.imp().swatches.clone();
        for (label, class_name, hue, sat) in PRESETS {
            let button = gtk::Button::new();
            button.add_css_class("color-swatch");
            button.add_css_class(class_name);
            button.set_tooltip_text(Some(label));
            button.set_accessible_role(gtk::AccessibleRole::Button);
            button.update_property(&[gtk::accessible::Property::Label(label)]);
            let popover = self.downgrade();
            let hue = *hue;
            let sat = *sat;
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
                .title("Custom Color")
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

impl Default for ColorPopover {
    fn default() -> Self {
        Self::new()
    }
}
