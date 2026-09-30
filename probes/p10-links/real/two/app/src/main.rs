fn main() {
    #[cfg(feature = "native")]
    {
        i18n::install();
        println!("{}", i18n::tr!("hi", name = "Ada"));
    }
}
