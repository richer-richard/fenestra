//! [`field`]: a form-field wrapper — a label (with an optional required mark)
//! above a control, and help or error text below it.
//!
//! ```
//! use fenestra_kit::{field, text_input};
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Email(String),
//! }
//!
//! let el: fenestra_core::Element<Msg> = field("Email")
//!     .required(true)
//!     .help("We'll never share it.")
//!     .child(text_input("").on_input(Msg::Email))
//!     .into();
//! ```

use fenestra_core::{Element, SP1, TextSize, Theme, Weight, col, row, text};

/// A form field under construction; converts into an [`Element`].
pub struct Field<Msg> {
    label: String,
    control: Option<Element<Msg>>,
    help: Option<String>,
    error: Option<String>,
    required: bool,
}

/// A labelled form field. Add a control with [`Field::child`], guidance with
/// [`Field::help`], and a validation message with [`Field::error`] (which
/// replaces the help text and reads in the danger tone).
pub fn field<Msg>(label: impl Into<String>) -> Field<Msg> {
    Field {
        label: label.into(),
        control: None,
        help: None,
        error: None,
        required: false,
    }
}

impl<Msg> Field<Msg> {
    /// The control this field wraps (input, select, switch, …).
    #[must_use]
    pub fn child(mut self, control: impl Into<Element<Msg>>) -> Self {
        self.control = Some(control.into());
        self
    }

    /// Muted helper text shown below the control (hidden when an error is set).
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// A validation message shown below the control in the danger tone; takes
    /// precedence over [`Field::help`].
    #[must_use]
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Append a danger-toned `*` to the label.
    #[must_use]
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Shows a [`Validity`](crate::validation::Validity)'s failing message as the
    /// field error (a no-op when valid). Pair with `.invalid(!v.valid)` on the
    /// wrapped control for the matching danger ring.
    #[must_use]
    pub fn validity(mut self, v: &super::validation::Validity) -> Self {
        if let Some(message) = &v.message {
            self.error = Some(message.clone());
        }
        self
    }
}

impl<Msg> From<Field<Msg>> for Element<Msg> {
    fn from(f: Field<Msg>) -> Self {
        let mut header: Vec<Element<Msg>> = vec![
            text(f.label)
                .size(TextSize::Sm)
                .weight(Weight::Medium)
                .themed(|t: &Theme, s| s.color(t.text)),
        ];
        if f.required {
            header.push(
                text("*")
                    .size(TextSize::Sm)
                    .weight(Weight::Medium)
                    .themed(|t: &Theme, s| s.color(t.danger.solid)),
            );
        }

        let mut kids: Vec<Element<Msg>> = vec![row().items_center().gap(2.0).children(header)];
        if let Some(control) = f.control {
            kids.push(control);
        }
        // The error message wins over help, and reads in the danger tone.
        if let Some(err) = f.error {
            kids.push(
                text(err)
                    .size(TextSize::Xs)
                    .themed(|t: &Theme, s| s.color(t.danger.solid)),
            );
        } else if let Some(help) = f.help {
            kids.push(
                text(help)
                    .size(TextSize::Xs)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            );
        }

        col().gap(SP1).children(kids)
    }
}
