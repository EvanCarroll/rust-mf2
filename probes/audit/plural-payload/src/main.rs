use icu_plurals::provider::{Baked, PluralsCardinalV1, PluralsOrdinalV1};
use icu_provider::prelude::*;
use icu_locale_core::locale;

fn size<M: DataMarker>(loc: &DataLocale) -> Option<usize>
where Baked: DataProvider<M>, for<'a> <M::DataStruct as icu_provider::prelude::yoke::Yokeable<'a>>::Output: serde::Serialize {
    let req = DataRequest { id: DataIdentifierBorrowed::for_locale(loc), ..Default::default() };
    let resp = DataProvider::<M>::load(&Baked, req).ok()?;
    Some(postcard::to_allocvec(resp.payload.get()).unwrap().len())
}

fn main() {
    for l in [locale!("en"), locale!("fr"), locale!("ru"), locale!("ar"), locale!("pl"), locale!("ja"), locale!("cy"), locale!("he"), locale!("sl")] {
        let dl = DataLocale::from(&l);
        println!("{l}: cardinal={:?} bytes ordinal={:?} bytes (postcard PluralRulesData)", size::<PluralsCardinalV1>(&dl), size::<PluralsOrdinalV1>(&dl));
    }
}
