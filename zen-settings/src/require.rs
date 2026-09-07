use adw::prelude::*;
use gtk::{Align, Orientation};

// What a page needs before its controls mean anything.
//
// A settings page for something you have not installed used to be an empty page with
// one line of prose, which tells you nothing about what the page would have offered.
// Instead the controls stay on screen and go insensitive, with a banner above them
// naming exactly what to install. You can see what you would get, and you cannot
// change something that is not there.
pub struct Need {
    pub command: &'static str,
    pub what: &'static str,
    pub package: &'static str,
}

pub fn missing(needs: &[Need]) -> Vec<&Need> {
    needs.iter().filter(|n| !have(n.command)).collect()
}

pub fn have(command: &str) -> bool {
    // PATH is what actually decides whether a spawn works, so it is what gets asked.
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let path = dir.join(command);
                path.is_file() || path.is_symlink()
            })
        })
        .unwrap_or(false)
}

// The banner, or None when nothing is missing.
pub fn banner(missing: &[&Need]) -> Option<gtk::Widget> {
    if missing.is_empty() {
        return None;
    }

    let card = gtk::Box::new(Orientation::Vertical, 6);
    card.add_css_class("notice");

    let headline = if missing.len() == 1 {
        format!("{} is not installed", missing[0].command)
    } else {
        "Some of this is not installed".to_owned()
    };
    card.append(
        &gtk::Label::builder()
            .label(headline)
            .halign(Align::Start)
            .css_classes(["notice-title"])
            .build(),
    );

    for need in missing {
        card.append(
            &gtk::Label::builder()
                .label(format!("{} — {}", need.command, need.what))
                .halign(Align::Start)
                .xalign(0.)
                .wrap(true)
                .css_classes(["setting-hint"])
                .build(),
        );
    }

    // One line to paste. Selectable, because the alternative is retyping it from a
    // screenshot of your own settings app.
    let packages: Vec<&str> = missing.iter().map(|n| n.package).collect();
    let command = gtk::Label::builder()
        .label(format!("sudo pacman -S {}", packages.join(" ")))
        .halign(Align::Start)
        .selectable(true)
        .css_classes(["mono"])
        .build();
    card.append(&command);

    card.append(
        &gtk::Label::builder()
            .label(
                "On another distribution the names may differ; ./setup.sh --check names \
                 the right ones for yours.",
            )
            .halign(Align::Start)
            .xalign(0.)
            .wrap(true)
            .css_classes(["setting-hint"])
            .build(),
    );

    Some(card.upcast())
}

// Puts the banner at the top of a page and greys out everything that follows.
//
// Returns whether anything was missing, so a caller that also wants to skip work can
// ask without checking twice.
pub fn guard(column: &gtk::Box, body: &gtk::Widget, needs: &[Need]) -> bool {
    let missing = missing(needs);
    if let Some(banner) = banner(&missing) {
        column.append(&banner);
    }

    // Insensitive rather than hidden: the point is to show what the page would do.
    body.set_sensitive(missing.is_empty());
    !missing.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_certain_to_exist_is_found() {
        assert!(have("sh"), "sh should be on PATH");
    }

    #[test]
    fn something_certain_not_to_exist_is_not() {
        assert!(!have("zen-definitely-not-a-real-command"));
    }

    #[test]
    fn only_the_absent_are_reported() {
        let needs = [
            Need {
                command: "sh",
                what: "a shell",
                package: "bash",
            },
            Need {
                command: "zen-definitely-not-a-real-command",
                what: "nothing",
                package: "nothing",
            },
        ];
        let gone = missing(&needs);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].command, "zen-definitely-not-a-real-command");
    }

    #[test]
    fn nothing_missing_means_no_banner() {
        assert!(banner(&[]).is_none());
    }
}
