pub enum Kind {
    Flag { invert: bool },
    Number { min: f64, max: f64, step: f64, digits: usize, default: f64 },
    Prop { prop: &'static str, min: f64, max: f64, default: f64 },
    Bool { default: bool },
    Color { fallback: &'static str },
    Text { fallback: &'static str, hint: &'static str },
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
                        hint: "How far the frost pulls toward the tint colour. 0 leaves the blur untinted",
                        path: &["glass"],
                        key: "opacity",
                        kind: Kind::Number { min: 0., max: 1., step: 0.01, default: 0.30, digits: 2 },
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
                        kind: Kind::Number { min: 0., max: 64., step: 1., default: 12., digits: 0 },
                    },
                    Row {
                        label: "Falloff",
                        hint: "How far in that bend reaches. Large values warp the whole surface",
                        path: &["glass"],
                        key: "falloff",
                        kind: Kind::Number { min: 2., max: 120., step: 1., default: 18., digits: 0 },
                    },
                    Row {
                        label: "Corner shape",
                        hint: "2 is a circular arc, 4 to 5 is continuous curvature",
                        path: &["glass"],
                        key: "squircle",
                        kind: Kind::Number { min: 2., max: 8., step: 0.1, default: 4.5, digits: 1 },
                    },
                ],
            },
            Group {
                title: "FROST",
                rows: &[
                    Row {
                        label: "Blur",
                        hint: "Off leaves refraction and tint on their own, which is as close to                                clear glass as the material goes",
                        path: &["blur"],
                        key: "off",
                        kind: Kind::Flag { invert: true },
                    },
                    Row {
                        label: "Passes",
                        hint: "Each pass roughly doubles the blur radius, and costs a little more",
                        path: &["blur"],
                        key: "passes",
                        kind: Kind::Number { min: 1., max: 6., step: 1., default: 3., digits: 0 },
                    },
                    Row {
                        label: "Spread",
                        hint: "Sample distance within a pass. Lower is a tighter, cleaner frost",
                        path: &["blur"],
                        key: "offset",
                        kind: Kind::Number { min: 1., max: 12., step: 0.5, default: 3., digits: 1 },
                    },
                    Row {
                        label: "Noise",
                        hint: "Breaks up the banding a heavy blur leaves on gradients",
                        path: &["blur"],
                        key: "noise",
                        kind: Kind::Number { min: 0., max: 0.2, step: 0.005, default: 0.02, digits: 3 },
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
                        kind: Kind::Number { min: 0.5, max: 2.5, step: 0.05, default: 1.3, digits: 2 },
                    },
                    Row {
                        label: "Specular",
                        hint: "Brightness of the lit edge",
                        path: &["glass"],
                        key: "specular",
                        kind: Kind::Number { min: 0., max: 0.6, step: 0.01, default: 0.12, digits: 2 },
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
                    kind: Kind::Number { min: 0., max: 64., step: 1., default: 20., digits: 0 },
                }],
            },
            Group {
                title: "EDGE, ON THE FOCUSED WINDOW",
                rows: &[
                    Row {
                        label: "Show a focus ring",
                        hint: "Drawn only around the window that has focus",
                        path: &["layout", "focus-ring"],
                        key: "off",
                        kind: Kind::Flag { invert: true },
                    },
                    Row {
                        label: "Width",
                        hint: "A thin edge reads as a rim light, a thick one as a highlighter",
                        path: &["layout", "focus-ring"],
                        key: "width",
                        kind: Kind::Number { min: 0., max: 8., step: 0.5, default: 2., digits: 1 },
                    },
                    Row {
                        label: "Colour",
                        hint: "",
                        path: &["layout", "focus-ring"],
                        key: "active-color",
                        kind: Kind::Color { fallback: "#cfe6ffcc" },
                    },
                    Row {
                        label: "Colour when unfocused",
                        hint: "Only visible on your other monitors",
                        path: &["layout", "focus-ring"],
                        key: "inactive-color",
                        kind: Kind::Color { fallback: "#ffffff1f" },
                    },
                ],
            },
            Group {
                title: "EDGE, ON EVERY WINDOW",
                rows: &[
                    Row {
                        label: "Show a border",
                        hint: "Always drawn, focused or not. Turn the focus ring off if you use this",
                        path: &["layout", "border"],
                        key: "off",
                        kind: Kind::Flag { invert: true },
                    },
                    Row {
                        label: "Width",
                        hint: "",
                        path: &["layout", "border"],
                        key: "width",
                        kind: Kind::Number { min: 0., max: 8., step: 0.5, default: 4., digits: 1 },
                    },
                    Row {
                        label: "Colour when focused",
                        hint: "",
                        path: &["layout", "border"],
                        key: "active-color",
                        kind: Kind::Color { fallback: "#ffc87f" },
                    },
                    Row {
                        label: "Colour when not",
                        hint: "",
                        path: &["layout", "border"],
                        key: "inactive-color",
                        kind: Kind::Color { fallback: "#ffffff1f" },
                    },
                ],
            },
            Group {
                title: "TINT",
                rows: &[
                    Row {
                        label: "Tint the whole window",
                        hint: "Off draws the focus ring and border as an outline instead of a                                colour cast behind the window",
                        path: &["window-rule"],
                        key: "draw-border-with-background",
                        kind: Kind::Bool { default: false },
                    },
                    Row {
                        label: "Focused transparency",
                        hint: "1 is opaque. The material behind a window only shows through the                                pixels the window leaves see-through",
                        path: &["window-rule@is-active=true"],
                        key: "opacity",
                        kind: Kind::Number { min: 0.3, max: 1., step: 0.01, default: 0.92, digits: 2 },
                    },
                    Row {
                        label: "Unfocused transparency",
                        hint: "Lower lets unfocused windows recede into the canvas",
                        path: &["window-rule@is-active=false"],
                        key: "opacity",
                        kind: Kind::Number { min: 0.3, max: 1., step: 0.01, default: 0.82, digits: 2 },
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
                        kind: Kind::Number { min: 0., max: 90., step: 1., default: 34., digits: 0 },
                    },
                    Row {
                        label: "Spread",
                        hint: "",
                        path: &["layout", "shadow"],
                        key: "spread",
                        kind: Kind::Number { min: 0., max: 24., step: 1., default: 2., digits: 0 },
                    },
                    Row {
                        label: "Drop",
                        hint: "How far the shadow falls below the window",
                        path: &["layout", "shadow"],
                        key: "offset",
                        kind: Kind::Prop { prop: "y", min: -20., default: 10., max: 40. },
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
                    kind: Kind::Number { min: 0.05, max: 1., step: 0.05, default: 0.2, digits: 2 },
                },
                Row {
                    label: "Maximum",
                    hint: "Above 1 magnifies",
                    path: &["camera"],
                    key: "max-zoom",
                    kind: Kind::Number { min: 1., max: 16., step: 0.5, default: 4., digits: 1 },
                },
                Row {
                    label: "Step",
                    hint: "Multiplier per zoom step",
                    path: &["camera"],
                    key: "zoom-step",
                    kind: Kind::Number { min: 1.05, max: 2., step: 0.05, default: 1.1, digits: 2 },
                },
            ],
        }],
    },
    Page {
        name: "Input",
        title: "Input",
        blurb: "How the pointer and the keyboard decide what is focused.",
        groups: &[
            Group {
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
            },
            Group {
                title: "KEYBOARD",
                rows: &[
                    Row {
                        label: "Hold before repeating",
                        hint: "Milliseconds a key must be held before it starts repeating",
                        path: &["input", "keyboard"],
                        key: "repeat-delay",
                        kind: Kind::Number { min: 100., max: 1200., step: 10., default: 600., digits: 0 },
                    },
                    Row {
                        label: "Repeat speed",
                        hint: "Repeats per second once it starts",
                        path: &["input", "keyboard"],
                        key: "repeat-rate",
                        kind: Kind::Number { min: 1., max: 100., step: 1., default: 25., digits: 0 },
                    },
                ],
            },
            Group {
                title: "POINTER",
                rows: &[
                    Row {
                        label: "Speed",
                        hint: "Acceleration, from -1 for slow and steady to 1 for quick",
                        path: &["input", "mouse"],
                        key: "accel-speed",
                        kind: Kind::Number { min: -1., max: 1., step: 0.05, default: 0., digits: 2 },
                    },
                    Row {
                        label: "Cursor size",
                        hint: "",
                        path: &["cursor"],
                        key: "xcursor-size",
                        kind: Kind::Number { min: 12., max: 96., step: 2., default: 24., digits: 0 },
                    },
                    Row {
                        label: "Cursor theme",
                        hint: "An XCursor theme installed on this system, such as Adwaita",
                        path: &["cursor"],
                        key: "xcursor-theme",
                        kind: Kind::Text { fallback: "default", hint: "default" },
                    },
                    Row {
                        label: "Hide while typing",
                        hint: "",
                        path: &["cursor"],
                        key: "hide-when-typing",
                        kind: Kind::Flag { invert: false },
                    },
                ],
            },
        ],
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
