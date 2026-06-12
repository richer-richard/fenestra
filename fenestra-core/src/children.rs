//! Heterogeneous children without ceremony: `.children(...)` accepts
//! both iterators of one convertible type *and* tuples mixing types —
//! `col().children((text("Hi"), button("Go"), row()))` just works, no
//! `Element::from` wrapping, no macros.
//!
//! The two source shapes are disambiguated by a marker type parameter
//! (the axum-handler trick), so the blanket iterator impl and the tuple
//! impls never overlap. Callers never name the marker — inference picks
//! the only impl that fits.

use crate::element::Element;

/// Anything that can become a child list. Implemented for every
/// `IntoIterator` of one `Into<Element>` type (vecs, arrays, `map`
/// chains) and for tuples of up to twelve *different* `Into<Element>`
/// types. `Marker` disambiguates the two families; let inference fill
/// it.
pub trait IntoChildren<Msg, Marker> {
    /// The realized child list, in order.
    fn into_children(self) -> Vec<Element<Msg>>;
}

/// Marker for the iterator family (see [`IntoChildren`]).
pub struct FromIter;

/// Marker for the tuple family (see [`IntoChildren`]).
pub struct FromTuple;

impl<Msg, I, T> IntoChildren<Msg, FromIter> for I
where
    I: IntoIterator<Item = T>,
    T: Into<Element<Msg>>,
{
    fn into_children(self) -> Vec<Element<Msg>> {
        self.into_iter().map(Into::into).collect()
    }
}

macro_rules! tuple_children {
    ($($t:ident),+) => {
        impl<Msg, $($t: Into<Element<Msg>>),+> IntoChildren<Msg, FromTuple> for ($($t,)+) {
            #[expect(
                non_snake_case,
                reason = "macro_rules can only bind tuple fields by their type-parameter names"
            )]
            fn into_children(self) -> Vec<Element<Msg>> {
                let ($($t,)+) = self;
                vec![$($t.into()),+]
            }
        }
    };
}

tuple_children!(A);
tuple_children!(A, B);
tuple_children!(A, B, C);
tuple_children!(A, B, C, D);
tuple_children!(A, B, C, D, E);
tuple_children!(A, B, C, D, E, F);
tuple_children!(A, B, C, D, E, F, G);
tuple_children!(A, B, C, D, E, F, G, H);
tuple_children!(A, B, C, D, E, F, G, H, I);
tuple_children!(A, B, C, D, E, F, G, H, I, J);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K, L);
