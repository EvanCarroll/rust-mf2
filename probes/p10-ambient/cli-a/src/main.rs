//! 1.x: an app-owned `NativeI18n`, the language from the system or the
//! first argument, three messages printed.
use probe_i18n::tr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut i18n = mf2_native::NativeI18n::embedded(&probe_i18n::CORPUS)?;
    if let Some(lang) = std::env::args().nth(1) {
        i18n.set_locale(&lang)?;
    }
    let sent: u32 = std::env::args().count().try_into()?;
    println!("{}", i18n.format(&tr!("app.title")));
    println!("{}", i18n.format(&tr!("status.target", host = "example.org")));
    println!("{}", i18n.format(&tr!("status.probes", n = sent)));
    Ok(())
}
