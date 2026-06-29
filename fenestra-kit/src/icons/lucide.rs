//! A vendored subset of [Lucide](https://lucide.dev) icons (ISC license,
//! see `LICENSE-LUCIDE.txt`): 24x24 stroked paths, stroke width 2, round
//! caps and joins, painted in the resolved text color like every fenestra
//! path element. Size them with `.w`/`.h` (24x24 intrinsic) and color them
//! with `.themed(|t, s| s.color(...))`.

mod data;

use fenestra_core::{Element, path};
use kurbo::BezPath;

fn icon<Msg>(d: &str) -> Element<Msg> {
    let bez = BezPath::from_svg(d).unwrap_or_default();
    path(bez, (24.0, 24.0), Some(2.0))
}

/// Every vendored icon as `(lucide name, element)`, in vendor order —
/// handy for icon pickers and gallery grids.
pub fn all<Msg>() -> impl Iterator<Item = (&'static str, Element<Msg>)> {
    data::ALL.iter().map(|(name, d)| (*name, icon(d)))
}

/// The element for a vendored icon by its kebab-case Lucide name, or `None` for
/// an unknown name — building only the matched icon. The by-name companion to
/// [`all`], so a name-driven consumer (e.g. JSON authoring) stays in sync with
/// the vendored set automatically.
pub fn by_name<Msg>(name: &str) -> Option<Element<Msg>> {
    data::ALL
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, d)| icon(d))
}

/// Every vendored icon's kebab-case name, in vendor order — for pickers, error
/// messages, and grammar docs, without building any elements.
pub fn names() -> impl Iterator<Item = &'static str> {
    data::ALL.iter().map(|(name, _)| *name)
}

macro_rules! lucide_icons {
    ($($fn_name:ident => $const_name:ident, $lucide:literal;)*) => {
        $(
            #[doc = concat!("The Lucide \"", $lucide, "\" icon (24x24, stroked).")]
            pub fn $fn_name<Msg>() -> Element<Msg> {
                icon(data::$const_name)
            }
        )*
    };
}

lucide_icons! {
    arrow_left => ARROW_LEFT, "arrow-left";
    arrow_right => ARROW_RIGHT, "arrow-right";
    bell => BELL, "bell";
    calendar => CALENDAR, "calendar";
    clock => CLOCK, "clock";
    copy => COPY, "copy";
    download => DOWNLOAD, "download";
    external_link => EXTERNAL_LINK, "external-link";
    eye => EYE, "eye";
    home => HOUSE, "house";
    info => INFO, "info";
    mail => MAIL, "mail";
    menu => MENU, "menu";
    minus => MINUS, "minus";
    moon => MOON, "moon";
    pencil => PENCIL, "pencil";
    plus => PLUS, "plus";
    search => SEARCH, "search";
    settings => SETTINGS, "settings";
    sun => SUN, "sun";
    alert_triangle => TRIANGLE_ALERT, "triangle-alert";
    trash => TRASH_2, "trash-2";
    upload => UPLOAD, "upload";
    user => USER, "user";
check => CHECK, "check";
    chevron_down => CHEVRON_DOWN, "chevron-down";
    chevron_left => CHEVRON_LEFT, "chevron-left";
    chevron_right => CHEVRON_RIGHT, "chevron-right";
    chevron_up => CHEVRON_UP, "chevron-up";
    file => FILE, "file";
    folder => FOLDER, "folder";
    link => LINK, "link";
    lock => LOCK, "lock";
    log_out => LOG_OUT, "log-out";
    refresh_cw => REFRESH_CW, "refresh-cw";
    save => SAVE, "save";
    star => STAR, "star";
    x => X, "x";
    calendar_days => CALENDAR_DAYS, "calendar-days";
    filter => FILTER, "filter";
    heart => HEART, "heart";
    share_2 => SHARE_2, "share-2";
}

#[cfg(test)]
mod tests {
    use kurbo::BezPath;

    /// Every vendored path parses and produces segments; a silently empty
    /// icon would otherwise only show up as a blank in the grid golden.
    #[test]
    fn every_icon_parses_non_empty() {
        for (name, d) in super::data::ALL {
            let bez = BezPath::from_svg(d).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(bez.segments().count() > 0, "{name} parsed to an empty path");
        }
    }
}
