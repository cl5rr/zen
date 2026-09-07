use adw::prelude::*;
use gtk::{Align, Orientation};

pub struct Need {
    pub command: &'static str,
    pub what: &'static str,
    pub package: &'static str,
    pub probe: Probe,
}

pub enum Probe {
    OnPath,
    AnyFile(&'static [&'static str]),
}

pub fn present(need: &Need) -> bool {
    match need.probe {
        Probe::OnPath => have(need.command),
        Probe::AnyFile(paths) => {
            have(need.command) || paths.iter().any(|p| std::path::Path::new(p).exists())
        }
    }
}

pub fn missing(needs: &[Need]) -> Vec<&Need> {
    needs.iter().filter(|n| !present(n)).collect()
}

pub fn have(command: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let path = dir.join(command);
                path.is_file() || path.is_symlink()
            })
        })
        .unwrap_or(false)
}

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
                .label(format!("{}: {}", need.command, need.what))
                .halign(Align::Start)
                .xalign(0.)
                .wrap(true)
                .css_classes(["setting-hint"])
                .build(),
        );
    }

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

pub fn guard(column: &gtk::Box, body: &gtk::Widget, needs: &[Need]) -> bool {
    let missing = missing(needs);
    if let Some(banner) = banner(&missing) {
        column.append(&banner);
    }

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
                probe: Probe::OnPath,
            },
            Need {
                command: "zen-definitely-not-a-real-command",
                what: "nothing",
                package: "nothing",
                probe: Probe::OnPath,
            },
        ];
        let gone = missing(&needs);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].command, "zen-definitely-not-a-real-command");
    }

    #[test]
    fn a_file_probe_finds_what_a_path_probe_cannot() {
        let need = Need {
            command: "zen-definitely-not-a-real-command",
            what: "something installed as a service",
            package: "nothing",
            probe: Probe::AnyFile(&["/etc/passwd"]),
        };
        assert!(present(&need), "an existing file should count as installed");
    }

    #[test]
    fn a_file_probe_still_reports_a_genuine_absence() {
        let need = Need {
            command: "zen-definitely-not-a-real-command",
            what: "nothing",
            package: "nothing",
            probe: Probe::AnyFile(&["/definitely/not/here"]),
        };
        assert!(!present(&need));
    }

    #[test]
    fn nothing_missing_means_no_banner() {
        assert!(banner(&[]).is_none());
    }
}
