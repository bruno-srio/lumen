use gettextrs::gettext;

/// Symbolic icons a group can show beside its name.
pub struct GroupIcon {
    pub id: &'static str,
}

pub const DEFAULT_GROUP_ICON: &str = "lumen-lamp-symbolic";

pub const GROUP_ICONS: &[GroupIcon] = &[
    GroupIcon {
        id: "lumen-lamp-symbolic",
    },
    GroupIcon {
        id: "lumen-home-symbolic",
    },
    GroupIcon {
        id: "lumen-bed-symbolic",
    },
    GroupIcon {
        id: "lumen-sofa-symbolic",
    },
    GroupIcon {
        id: "lumen-desk-symbolic",
    },
    GroupIcon {
        id: "lumen-kitchen-symbolic",
    },
    GroupIcon {
        id: "lumen-bath-symbolic",
    },
    GroupIcon {
        id: "lumen-sun-symbolic",
    },
];

impl GroupIcon {
    pub fn label(&self) -> String {
        match self.id {
            "lumen-home-symbolic" => gettext("Home"),
            "lumen-bed-symbolic" => gettext("Bed"),
            "lumen-sofa-symbolic" => gettext("Sofa"),
            "lumen-desk-symbolic" => gettext("Desk"),
            "lumen-kitchen-symbolic" => gettext("Kitchen"),
            "lumen-bath-symbolic" => gettext("Bath"),
            "lumen-sun-symbolic" => gettext("Sun"),
            _ => gettext("Lamp"),
        }
    }
}

pub fn normalize_group_icon(icon: &str) -> &'static str {
    GROUP_ICONS
        .iter()
        .find(|item| item.id == icon)
        .map(|item| item.id)
        .unwrap_or(DEFAULT_GROUP_ICON)
}

pub fn default_group_icon() -> String {
    DEFAULT_GROUP_ICON.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_icon_stays_itself() {
        assert_eq!(
            normalize_group_icon("lumen-bed-symbolic"),
            "lumen-bed-symbolic"
        );
    }

    #[test]
    fn unknown_icon_falls_back_to_the_lamp() {
        assert_eq!(normalize_group_icon(""), DEFAULT_GROUP_ICON);
        assert_eq!(normalize_group_icon("missing"), DEFAULT_GROUP_ICON);
    }
}
