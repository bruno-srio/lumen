/// Symbolic icons a group can show beside its name.
pub struct GroupIcon {
    pub id: &'static str,
    pub label: &'static str,
}

pub const DEFAULT_GROUP_ICON: &str = "lumen-lamp-symbolic";

pub const GROUP_ICONS: &[GroupIcon] = &[
    GroupIcon {
        id: "lumen-lamp-symbolic",
        label: "Lamp",
    },
    GroupIcon {
        id: "lumen-home-symbolic",
        label: "Home",
    },
    GroupIcon {
        id: "lumen-bed-symbolic",
        label: "Bed",
    },
    GroupIcon {
        id: "lumen-sofa-symbolic",
        label: "Sofa",
    },
    GroupIcon {
        id: "lumen-desk-symbolic",
        label: "Desk",
    },
    GroupIcon {
        id: "lumen-kitchen-symbolic",
        label: "Kitchen",
    },
    GroupIcon {
        id: "lumen-bath-symbolic",
        label: "Bath",
    },
    GroupIcon {
        id: "lumen-sun-symbolic",
        label: "Sun",
    },
];

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
