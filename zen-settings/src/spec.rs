pub enum Kind {
    Flag { invert: bool },
    Number { min: f64, max: f64, step: f64, digits: usize },
    Prop { prop: &'static str, min: f64, max: f64 },
    Color { fallback: &'static str },
}

pub struct Row {
    pub label: &'static str,
    pub hint: &'static str,
    pub path: &'static [&'static str],
    pub key: &'static str,
    pub kind: Kind,
}

pub struct Group {
    pub title: &'static str,
    pub rows: &'static [Row],
}

pub struct Page {
    pub name: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    pub groups: &'static [Group],
}

pub const PAGES: &[Page] = &[
    Page {
        name: "Material",
        title: "Material",
        blurb: "Glass is a rim effect. It only shows through pixels a window leaves translucent.",
        groups: &[
            Group {
                title: "GLASS",
                rows: &[
                    Row {
                        label: "Enabled",
                        hint: "Windows still ask for it with background-effect in a rule",
                        path: &["glass"],
                        key: "off",
                        kind: Kind::Flag { invert: true },
                    },
                    Row {
                        label: "Opacity",
                        hint: "0 is clear glass, 1 is a solid pane",
                        path: &["glass"],
                        key: "opacity",
                        kind: Kind::Number { min: 0., max: 1., step: 0.01, digits: 2 },
                    },
                    Row {
                        label: "Tint",
                        hint: "The colour the frost pulls toward",
                        path: &["glass"],
                        key: "tint",
                        kind: Kind::Color { fallback: "#ffffff" },
                    },
                ],
            },
            Group {
                title: "REFRACTION",
                rows: &[
                    Row {
                        label: "Strength",
                        hint: "How far the rim bends what is behind it, in pixels",
                        path: &["glass"],
                        key: "refraction",
                        kind: Kind::Number { min: 0., max: 64., step: 1., digits: 0 },
                    },
                    Row {
                        label: "Falloff",
                        hint: "How far in that bend reaches. Large values warp the whole surface",
                        path: &["glass"],
                        key: "falloff",
                        kind: Kind::Number { min: 2., max: 120., step: 1., digits: 0 },
                    },
                    Row {
                        label: "Corner shape",
                        hint: "2 is a circular arc, 4 to 5 is continuous curvature",
                        path: &["glass"],
                        key: "squircle",
                        kind: Kind::Number { min: 2., max: 8., step: 0.1, digits: 1 },
                    },
                ],
            },
            Group {
                title: "VIBRANCY",
                rows: &[
                    Row {
                        label: "Saturation",
                        hint: "Blur without a saturation lift reads as frosted plastic",
                        path: &["glass"],
                        key: "saturation",
                        kind: Kind::Number { min: 0.5, max: 2.5, step: 0.05, digits: 2 },
                    },
                    Row {
                        label: "Specular",
                        hint: "Brightness of the lit edge",
                        path: &["glass"],
                        key: "specular",
                        kind: Kind::Number { min: 0., max: 0.6, step: 0.01, digits: 2 },
                    },
                ],
            },
        ],
    },
    Page {
        name: "Windows",
        title: "Windows",
        blurb: "Spacing, edges and depth.",
        groups: &[
            Group {
                title: "SPACING",
                rows: &[Row {
                    label: "Gaps",
                    hint: "Logical pixels between windows and around them",
                    path: &["layout"],
                    key: "gaps",
                    kind: Kind::Number { min: 0., max: 64., step: 1., digits: 0 },
                }],
            },
            Group {
                title: "FOCUS RING",
                rows: &[
                    Row {
                        label: "Enabled",
                        hint: "",
                        path: &["layout", "focus-ring"],
                        key: "off",
                        kind: Kind::Flag { invert: true },
                    },
                    Row {
                        label: "Width",
                        hint: "A thin ring reads as a rim light, a thick one as a highlighter",
                        path: &["layout", "focus-ring"],
                        key: "width",
                        kind: Kind::Number { min: 0., max: 8., step: 0.5, digits: 1 },
                    },
                    Row {
                        label: "Colour",
                        hint: "",
                        path: &["layout", "focus-ring"],
                        key: "active-color",
                        kind: Kind::Color { fallback: "#cfe6ffcc" },
                    },
                ],
            },
            Group {
                title: "BORDER",
                rows: &[
                    Row {
                        label: "Enabled",
                        hint: "Always visible, unlike the focus ring",
                        path: &["layout", "border"],
                        key: "off",
                        kind: Kind::Flag { invert: true },
                    },
                    Row {
                        label: "Width",
                        hint: "",
                        path: &["layout", "border"],
                        key: "width",
                        kind: Kind::Number { min: 0., max: 8., step: 0.5, digits: 1 },
                    },
                    Row {
                        label: "Colour",
                        hint: "",
                        path: &["layout", "border"],
                        key: "active-color",
                        kind: Kind::Color { fallback: "#ffc87f" },
                    },
                ],
            },
            Group {
                title: "SHADOW",
                rows: &[
                    Row {
                        label: "Enabled",
                        hint: "",
                        path: &["layout", "shadow"],
                        key: "on",
                        kind: Kind::Flag { invert: false },
                    },
                    Row {
                        label: "Softness",
                        hint: "Blur radius, the same idea as a CSS box-shadow",
                        path: &["layout", "shadow"],
                        key: "softness",
                        kind: Kind::Number { min: 0., max: 90., step: 1., digits: 0 },
                    },
                    Row {
                        label: "Spread",
                        hint: "",
                        path: &["layout", "shadow"],
                        key: "spread",
                        kind: Kind::Number { min: 0., max: 24., step: 1., digits: 0 },
                    },
                    Row {
                        label: "Drop",
                        hint: "How far the shadow falls below the window",
                        path: &["layout", "shadow"],
                        key: "offset",
                        kind: Kind::Prop { prop: "y", min: -20., max: 40. },
                    },
                    Row {
                        label: "Colour",
                        hint: "",
                        path: &["layout", "shadow"],
                        key: "color",
                        kind: Kind::Color { fallback: "#00000099" },
                    },
                ],
            },
        ],
    },
    Page {
        name: "Camera",
        title: "Camera",
        blurb: "ZEN frames a window by moving the viewport, not by resizing it.",
        groups: &[Group {
            title: "ZOOM",
            rows: &[
                Row {
                    label: "Minimum",
                    hint: "How far out the camera can pull",
                    path: &["camera"],
                    key: "min-zoom",
                    kind: Kind::Number { min: 0.05, max: 1., step: 0.05, digits: 2 },
                },
                Row {
                    label: "Maximum",
                    hint: "Above 1 magnifies",
                    path: &["camera"],
                    key: "max-zoom",
                    kind: Kind::Number { min: 1., max: 16., step: 0.5, digits: 1 },
                },
                Row {
                    label: "Step",
                    hint: "Multiplier per zoom step",
                    path: &["camera"],
                    key: "zoom-step",
                    kind: Kind::Number { min: 1.05, max: 2., step: 0.05, digits: 2 },
                },
            ],
        }],
    },
    Page {
        name: "Input",
        title: "Input",
        blurb: "How the pointer and the keyboard decide what is focused.",
        groups: &[Group {
            title: "FOCUS",
            rows: &[
                Row {
                    label: "Follow the mouse",
                    hint: "On, moving the pointer onto a window focuses it. Off, you click to focus",
                    path: &["input"],
                    key: "focus-follows-mouse",
                    kind: Kind::Flag { invert: false },
                },
                Row {
                    label: "Warp the pointer to focus",
                    hint: "Jump the pointer to the centre of a window when it takes focus",
                    path: &["input"],
                    key: "warp-mouse-to-focus",
                    kind: Kind::Flag { invert: false },
                },
            ],
        }],
    },
    Page {
        name: "Startup",
        title: "Startup",
        blurb: "What you see in the first two seconds.",
        groups: &[Group {
            title: "WELCOME",
            rows: &[
                Row {
                    label: "Enabled",
                    hint: "The cover parts along the middle once, at startup",
                    path: &["welcome"],
                    key: "off",
                    kind: Kind::Flag { invert: true },
                },
                Row {
                    label: "Cover colour",
                    hint: "The mark is light, so a dark cover keeps it readable",
                    path: &["welcome"],
                    key: "color",
                    kind: Kind::Color { fallback: "#0b0c0e" },
                },
            ],
        }],
    },
];
