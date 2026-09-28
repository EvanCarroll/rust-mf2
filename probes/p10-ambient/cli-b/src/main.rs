//! The design: `install()` once, the language from the system or the first
//! argument, three messages printed through `Display`.
use ambient::Show;
use probe_i18n::tr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ambient::install(&probe_i18n::CORPUS)?;
    if let Some(lang) = std::env::args().nth(1) {
        ambient::set_locale(&lang)?;
    }
    let sent: u32 = std::env::args().count().try_into()?;
    println!("{}", Show(&tr!("app.title")));
    println!("{}", Show(&tr!("status.target", host = "example.org")));
    println!("{}", Show(&tr!("status.probes", n = sent)));
    Ok(())
}
