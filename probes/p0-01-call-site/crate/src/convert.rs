//! `From` conversions for component props and plain strings. Each is one
//! library function (one closure type), not one per call site.

use leptos::oco::Oco;
use leptos::prelude::{Signal, TextProp};

use crate::args::ArgValue;
use crate::catalog::track_locale;
use crate::format::{Tracking, resolve_string};
use crate::{MsgId, Tr, TrArgs};

#[inline(never)]
fn text_prop(id: MsgId, args: Box<[ArgValue]>) -> TextProp {
    TextProp::from(move || {
        track_locale();
        Oco::<'static, str>::Owned(resolve_string(id, &args, Tracking::Tracked))
    })
}

#[inline(never)]
fn signal_string(id: MsgId, args: Box<[ArgValue]>) -> Signal<String> {
    Signal::derive(move || {
        track_locale();
        resolve_string(id, &args, Tracking::Tracked)
    })
}

impl From<Tr> for TextProp {
    fn from(t: Tr) -> Self {
        text_prop(t.id, Box::default())
    }
}

impl From<TrArgs> for TextProp {
    fn from(t: TrArgs) -> Self {
        let (id, args) = t.into_parts();
        text_prop(id, args)
    }
}

impl From<Tr> for Signal<String> {
    fn from(t: Tr) -> Self {
        signal_string(t.id, Box::default())
    }
}

impl From<TrArgs> for Signal<String> {
    fn from(t: TrArgs) -> Self {
        let (id, args) = t.into_parts();
        signal_string(id, args)
    }
}

impl From<Tr> for Oco<'static, str> {
    fn from(t: Tr) -> Self {
        Oco::Owned(t.to_string())
    }
}

impl From<TrArgs> for Oco<'static, str> {
    fn from(t: TrArgs) -> Self {
        Oco::Owned(t.to_string())
    }
}

impl From<Tr> for String {
    fn from(t: Tr) -> Self {
        t.to_string()
    }
}

impl From<TrArgs> for String {
    fn from(t: TrArgs) -> Self {
        t.to_string()
    }
}
